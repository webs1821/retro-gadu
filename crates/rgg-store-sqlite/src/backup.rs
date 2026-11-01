// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Consistent SQLite snapshots and read-only validation; never initializes a backup as a runtime.
use crate::{SCHEMA_VERSION, SqliteStore, database as sqlx, migrations::EmbeddedMigrations};
use futures_util::TryStreamExt;
use rgg_application::accounts::{SealedSecret, SecretProtector};
use rgg_domain::UserId;
use sqlx::{
    ConnectOptions, Row,
    migrate::MigrationSource,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{collections::BTreeSet, path::Path};
pub const MAX_SNAPSHOT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub struct SnapshotInfo {
    pub schema: i64,
    pub accounts: u64,
    pub key_ids: Vec<String>,
}
impl SqliteStore {
    /// Caller owns the data-directory lock and an empty, private destination file.
    pub async fn snapshot_into(&self, path: &Path) -> Result<(), &'static str> {
        let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&self.pool)
            .await
            .map_err(|_| "snapshot unavailable")?;
        let size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&self.pool)
            .await
            .map_err(|_| "snapshot unavailable")?;
        if pages < 0 || size < 0 || pages.saturating_mul(size) as u64 > MAX_SNAPSHOT_BYTES {
            return Err("snapshot exceeds size limit");
        }
        sqlx::query("VACUUM INTO ?")
            .bind(path.to_str().ok_or("unsupported backup path")?)
            .execute(&self.writer)
            .await
            .map_err(|_| "snapshot creation failed")?;
        Ok(())
    }
}
pub async fn inspect_snapshot(
    path: &Path,
    protector: &dyn SecretProtector,
) -> Result<SnapshotInfo, &'static str> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .immutable(true)
        .pragma("trusted_schema", "OFF")
        .disable_statement_logging();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| "snapshot database unavailable")?;
    let result = validate(&pool, protector).await;
    pool.close().await;
    result
}
async fn validate(
    pool: &sqlx::SqlitePool,
    protector: &dyn SecretProtector,
) -> Result<SnapshotInfo, &'static str> {
    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await
        .map_err(|_| "snapshot integrity failed")?;
    if check != "ok"
        || sqlx::query("PRAGMA foreign_key_check")
            .fetch_optional(pool)
            .await
            .map_err(|_| "snapshot foreign keys failed")?
            .is_some()
    {
        return Err("snapshot integrity failed");
    }
    let recorded = sqlx::query(
        "SELECT version,success,checksum FROM _sqlx_migrations ORDER BY version LIMIT 100",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| "snapshot migration metadata invalid")?;
    let expected = EmbeddedMigrations
        .resolve()
        .await
        .map_err(|_| "migration metadata unavailable")?;
    if recorded.len() != expected.len() {
        return Err("unsupported snapshot schema");
    }
    for (row, migration) in recorded.iter().zip(&expected) {
        if row
            .try_get::<i64, _>("version")
            .map_err(|_| "snapshot migration metadata invalid")?
            != migration.version
            || !row
                .try_get::<bool, _>("success")
                .map_err(|_| "snapshot migration metadata invalid")?
            || row
                .try_get::<Vec<u8>, _>("checksum")
                .map_err(|_| "snapshot migration metadata invalid")?
                != migration.checksum.as_ref()
        {
            return Err("snapshot migration checksum mismatch");
        }
    }
    let invalid:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE length(key_id)>32 OR length(nonce)!=24 OR length(ciphertext) NOT BETWEEN 24 AND 31 OR secret_version!=1)").fetch_one(pool).await.map_err(|_|"snapshot credentials invalid")?;
    if invalid != 0 {
        return Err("snapshot credentials invalid");
    }
    let mut rows =
        sqlx::query("SELECT uin,secret_version,key_id,nonce,ciphertext FROM accounts ORDER BY uin")
            .fetch(pool);
    let mut accounts = 0;
    let mut key_ids = BTreeSet::new();
    while let Some(row) = rows
        .try_next()
        .await
        .map_err(|_| "snapshot credentials invalid")?
    {
        let uid: i64 = row
            .try_get("uin")
            .map_err(|_| "snapshot credentials invalid")?;
        let user = UserId::new(uid.try_into().map_err(|_| "snapshot credentials invalid")?)
            .map_err(|_| "snapshot credentials invalid")?;
        let secret = SealedSecret {
            version: 1,
            key_id: row
                .try_get("key_id")
                .map_err(|_| "snapshot credentials invalid")?,
            nonce: row
                .try_get::<Vec<u8>, _>("nonce")
                .map_err(|_| "snapshot credentials invalid")?
                .try_into()
                .map_err(|_| "snapshot credentials invalid")?,
            ciphertext: row
                .try_get("ciphertext")
                .map_err(|_| "snapshot credentials invalid")?,
        };
        protector
            .open(user, &secret)
            .map_err(|_| "snapshot keyring cannot decrypt every account")?;
        key_ids.insert(secret.key_id);
        accounts += 1;
    }
    Ok(SnapshotInfo {
        schema: SCHEMA_VERSION,
        accounts,
        key_ids: key_ids.into_iter().collect(),
    })
}
