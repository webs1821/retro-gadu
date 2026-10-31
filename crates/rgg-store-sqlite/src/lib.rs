// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

mod accounts;
mod database;
mod messaging;
#[cfg(test)]
mod messaging_tests;
mod migrations;
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

pub const SCHEMA_VERSION: i64 = 4;

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
}

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
    directory: Arc<DataDirectory>,
    epoch: String,
}

impl SqliteStore {
    pub async fn open(path: &Path) -> Result<Self, OpenError> {
        let directory = Arc::new(DataDirectory::acquire(path)?);
        let options = SqliteConnectOptions::new()
            .filename(directory.root().join("rgg.sqlite"))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .disable_statement_logging();
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(|connection, _| {
                Box::pin(async move {
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
            .connect_with(options)
            .await?;
        let result = Self::initialize(&pool).await;
        let epoch = match result {
            Ok(epoch) => epoch,
            Err(error) => {
                pool.close().await;
                return Err(error);
            }
        };
        Ok(Self {
            pool,
            directory,
            epoch,
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
            .execute(&store.pool)
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
