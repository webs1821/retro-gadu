// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

mod accounts;
pub mod backup;
#[cfg(test)]
mod conference_tests;
mod contacts;
mod database;
#[cfg(test)]
mod image_tests;
mod images;
pub mod import_data;
pub mod key_maintenance;
mod messaging;
#[cfg(test)]
mod messaging_tests;
mod migrations;
mod operations;
#[cfg(test)]
mod operations_tests;
pub mod password_maintenance;
mod password_reset;
mod password_web;
#[cfg(test)]
mod password_web_tests;
mod presence;
#[cfg(test)]
mod presence_tests;
#[cfg(test)]
mod profile_tests;
mod public_profiles;
mod statistics;
#[cfg(test)]
mod statistics_tests;
#[cfg(test)]
mod write_pool_tests;
use crate::database as sqlx;
pub mod directory;

use async_trait::async_trait;
use directory::{DataDirectory, DirectoryError};
use rand::TryRngCore;
use rgg_application::ports::{HealthRepository, StorageUnavailable};
use sqlx::{
    ConnectOptions, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, sync::Arc, time::Duration};

pub const SCHEMA_VERSION: i64 = 14;

// The bundled engine must include the WAL-reset fix. Check the linked runtime,
// not the version of a system sqlite3/Python command used by external diagnostics.
fn sqlite_version_supported(value: &str) -> bool {
    let mut parts = value.split('.');
    let mut version = [0_u32; 3];
    for component in &mut version {
        let Some(value) = parts.next().and_then(|part| part.parse().ok()) else {
            return false;
        };
        *component = value;
    }
    // 3.52 was withdrawn, not a supported successor to the fixed 3.51.3 release.
    parts.next().is_none() && version >= [3, 51, 3] && version[..2] != [3, 52]
}

#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error(transparent)]
    Directory(#[from] DirectoryError),
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("database schema is newer than this program")]
    NewerSchema,
    #[error("SQLite durability settings do not match WAL/FULL/foreign_keys")]
    Durability,
    #[error("secure random source unavailable")]
    Random,
    #[error("data directory contains an incomplete restore; use a fresh destination")]
    IncompleteRestore,
    #[error("data directory contains an incomplete import; use a fresh destination")]
    IncompleteImport,
}

#[derive(Clone)]
pub struct SqliteStore {
    // Readers cannot become writers. A single pooled writer provides FIFO acquisition
    // instead of making four SQLite workers compete in busy_timeout while holding slots.
    pool: SqlitePool,
    writer: SqlitePool,
    directory: Arc<DataDirectory>,
    epoch: String,
}

impl SqliteStore {
    pub async fn open(path: &Path) -> Result<Self, OpenError> {
        Self::open_mode(path, false).await
    }

    /// Initialize an import only in a marked directory without any database or keyring.
    /// This never resumes an interrupted import or opens an existing database.
    pub async fn open_new_import(path: &Path) -> Result<Self, OpenError> {
        Self::open_mode(path, true).await
    }

    async fn open_mode(path: &Path, new_import: bool) -> Result<Self, OpenError> {
        let directory = Arc::new(DataDirectory::acquire(path)?);
        let marked_import = directory
            .root()
            .join("import-incomplete")
            .try_exists()
            .map_err(DirectoryError::Io)?;
        if new_import {
            if !marked_import {
                return Err(OpenError::IncompleteImport);
            }
            for entry in std::fs::read_dir(directory.root()).map_err(DirectoryError::Io)? {
                let name = entry.map_err(DirectoryError::Io)?.file_name();
                if name != "rgg.lock" && name != "import-incomplete" {
                    return Err(OpenError::IncompleteImport);
                }
            }
        } else if marked_import {
            return Err(OpenError::IncompleteImport);
        }
        if directory
            .root()
            .join("restore-incomplete")
            .try_exists()
            .map_err(DirectoryError::Io)?
        {
            return Err(OpenError::IncompleteRestore);
        }
        let options = SqliteConnectOptions::new()
            .filename(directory.root().join("rgg.sqlite"))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let writer = Self::pool_options(1).connect_with(options.clone()).await?;
        let result = Self::initialize(&writer).await;
        let epoch = match result {
            Ok(epoch) => epoch,
            Err(error) => {
                writer.close().await;
                return Err(error);
            }
        };
        let readers = Self::pool_options(3)
            .connect_with(
                options
                    .create_if_missing(false)
                    .read_only(true)
                    .pragma("query_only", "ON"),
            )
            .await;
        let pool = match readers {
            Ok(pool) => pool,
            Err(error) => {
                writer.close().await;
                return Err(error.into());
            }
        };
        Ok(Self {
            pool,
            writer,
            directory,
            epoch,
        })
    }

    fn pool_options(max_connections: u32) -> SqlitePoolOptions {
        SqlitePoolOptions::new()
            .max_connections(max_connections)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
                    let version: String = sqlx::query_scalar("SELECT sqlite_version()")
                        .fetch_one(&mut *connection)
                        .await?;
                    if !sqlite_version_supported(&version) {
                        return Err(sqlx::Error::Protocol(
                            "SQLite 3.51.3+ required; withdrawn 3.52 excluded (WAL-reset fix)"
                                .into(),
                        ));
                    }
                    let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
                        .fetch_one(&mut *connection)
                        .await?;
                    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                        .fetch_one(&mut *connection)
                        .await?;
                    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
                        .fetch_one(&mut *connection)
                        .await?;
                    if synchronous != 2 || foreign_keys != 1 || journal != "wal" {
                        return Err(sqlx::Error::Protocol("SQLite durability mismatch".into()));
                    }
                    Ok(())
                })
            })
    }

    async fn initialize(pool: &SqlitePool) -> Result<String, OpenError> {
        let has_migrations: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
        )
        .fetch_one(pool)
        .await?;
        if has_migrations != 0 {
            let max: Option<i64> = sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations")
                .fetch_one(pool)
                .await?;
            if max.unwrap_or(0) > SCHEMA_VERSION {
                return Err(OpenError::NewerSchema);
            }
        }
        sqlx::migrate::Migrator::new(migrations::EmbeddedMigrations)
            .await?
            .run(pool)
            .await?;
        let mut entropy = [0u8; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut entropy)
            .map_err(|_| OpenError::Random)?;
        let epoch: String = entropy.iter().map(|byte| format!("{byte:02x}")).collect();
        let mut tx = pool.begin().await?;
        // The OS lock proves that every previous process/writer is dead. Wall-clock lease
        // expiry is deliberately not a prerequisite for recovery after a new epoch.
        sqlx::query("UPDATE delivery_attempts SET outcome='RecoveredEpoch',completed_at=unixepoch() WHERE outcome IS NULL").execute(&mut *tx).await?;
        sqlx::query("UPDATE deliveries SET state='Pending',lease_token=NULL,session_id=NULL,epoch=NULL,generation=NULL,lease_until=NULL WHERE state='Leased'").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM account_sessions")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM account_presence WHERE availability!='offline'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM contact_staging")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM admin_audit WHERE occurred_at<unixepoch()-2592000")
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO runtime_state(id,process_epoch,started_at) VALUES(1,?,unixepoch()) ON CONFLICT(id) DO UPDATE SET process_epoch=excluded.process_epoch,started_at=excluded.started_at")
            .bind(&epoch).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(epoch)
    }

    pub fn process_epoch(&self) -> &str {
        &self.epoch
    }
    pub fn data_directory(&self) -> &Path {
        self.directory.root()
    }
    pub async fn close(&self) {
        self.pool.close().await;
        self.writer.close().await;
    }

    pub async fn integrity_check(&self) -> Result<(), OpenError> {
        let result: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;
        if result != "ok" {
            return Err(OpenError::Database(sqlx::Error::Protocol(
                "integrity check failed".into(),
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl HealthRepository for SqliteStore {
    async fn check(&self) -> Result<(), StorageUnavailable> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|_| StorageUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_engine_floor_rejects_unfixed_and_unrecognized_versions() {
        for version in [
            "3.46.0",
            "3.51.2",
            "3.52.0",
            "",
            "3.51",
            "3.51.3.0",
            "unrecognized",
        ] {
            assert!(!sqlite_version_supported(version), "{version}");
        }
        for version in ["3.51.3", "3.53.0", "4.0.0"] {
            assert!(sqlite_version_supported(version), "{version}");
        }
    }

    #[tokio::test]
    async fn import_opener_requires_marker_and_cannot_reopen_existing_database() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            SqliteStore::open_new_import(root.path()).await,
            Err(OpenError::IncompleteImport)
        ));
        assert!(!root.path().join("rgg.sqlite").exists());
        std::fs::write(root.path().join("import-incomplete"), []).unwrap();
        let store = SqliteStore::open_new_import(root.path()).await.unwrap();
        store.close().await;
        drop(store);
        assert!(matches!(
            SqliteStore::open(root.path()).await,
            Err(OpenError::IncompleteImport)
        ));
        assert!(matches!(
            SqliteStore::open_new_import(root.path()).await,
            Err(OpenError::IncompleteImport)
        ));
    }

    #[tokio::test]
    async fn initialization_is_durable_and_each_restart_gets_a_new_epoch() {
        let root = tempfile::tempdir().unwrap();
        let first = SqliteStore::open(root.path()).await.unwrap();
        first.check().await.unwrap();
        first.integrity_check().await.unwrap();
        let epoch = first.process_epoch().to_owned();
        assert!(matches!(
            SqliteStore::open(root.path()).await,
            Err(OpenError::Directory(DirectoryError::InUse))
        ));
        first.close().await;
        drop(first);
        let second = SqliteStore::open(root.path()).await.unwrap();
        assert_ne!(epoch, second.process_epoch());
        let migrations: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&second.pool)
            .await
            .unwrap();
        assert_eq!(migrations, SCHEMA_VERSION);
        second.close().await;
    }

    #[tokio::test]
    async fn refuses_future_schema_without_overwriting_runtime_state() {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(root.path()).await.unwrap();
        let epoch = store.process_epoch().to_owned();
        sqlx::query("UPDATE _sqlx_migrations SET version=9999 WHERE version=(SELECT max(version) FROM _sqlx_migrations)")
            .execute(&store.writer)
            .await
            .unwrap();
        store.close().await;
        drop(store);
        assert!(matches!(
            SqliteStore::open(root.path()).await,
            Err(OpenError::NewerSchema)
        ));
        let connection = SqlitePool::connect_with(
            SqliteConnectOptions::new()
                .filename(root.path().join("rgg.sqlite"))
                .read_only(true),
        )
        .await
        .unwrap();
        let saved: String =
            sqlx::query_scalar("SELECT process_epoch FROM runtime_state WHERE id=1")
                .fetch_one(&connection)
                .await
                .unwrap();
        assert_eq!(saved, epoch);
        connection.close().await;
    }
}
