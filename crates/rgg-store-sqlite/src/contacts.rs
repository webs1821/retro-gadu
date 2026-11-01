// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{
    SqliteStore, database as sqlx,
    messaging::{db, fence},
};
use async_trait::async_trait;
use rgg_application::{
    contacts::*,
    messaging::{MessagingError, SessionKey},
};
use sqlx::Row;
#[async_trait]
impl ContactArchiveRepository for SqliteStore {
    async fn stage_archive(
        &self,
        key: &SessionKey,
        segment: ArchiveSegment,
    ) -> Result<ArchiveResult, MessagingError> {
        if segment.text.len() > 6141
            || segment.text.chars().count() > 2047
            || segment.text.contains('\0')
        {
            return Err(MessagingError::Invalid);
        }
        let mut tx = self.writer.begin().await.map_err(db)?;
        fence(&mut tx, key, true).await?;
        let current: Option<i64> =
            sqlx::query_scalar("SELECT version FROM contact_archives WHERE owner=?")
                .bind(i64::from(key.user.get()))
                .fetch_optional(&mut *tx)
                .await
                .map_err(db)?;
        let version = current.unwrap_or(0);
        if segment.start {
            sqlx::query("DELETE FROM contact_staging WHERE owner=?")
                .bind(i64::from(key.user.get()))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            sqlx::query("INSERT INTO contact_staging(owner,session_id,epoch,generation,base_version,document) VALUES(?,?,?,?,?,'')").bind(i64::from(key.user.get())).bind(key.id.as_slice()).bind(&key.epoch).bind(key.generation).bind(version).execute(&mut *tx).await.map_err(db)?;
        }
        let row=sqlx::query("SELECT document,base_version FROM contact_staging WHERE owner=? AND session_id=? AND epoch=? AND generation=?")
            .bind(i64::from(key.user.get())).bind(key.id.as_slice()).bind(&key.epoch).bind(key.generation).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(MessagingError::Invalid)?;
        if row.get::<i64, _>("base_version") != version {
            return Err(MessagingError::Stale);
        }
        let mut document: String = row.get("document");
        document.push_str(&segment.text);
        #[cfg(feature = "test-hooks")]
        let crash_marker = segment.complete && document == "rGG_archive_fault";
        validate_archive(&document)?;
        let bytes:i64=sqlx::query_scalar("SELECT (SELECT COALESCE(sum(length(CAST(document AS BLOB))),0) FROM contact_archives)+(SELECT COALESCE(sum(length(CAST(document AS BLOB))),0) FROM contact_staging WHERE owner!=?)")
            .bind(i64::from(key.user.get())).fetch_one(&mut *tx).await.map_err(db)?;
        if bytes + document.len() as i64 > 96 * 1024 * 1024 {
            return Err(MessagingError::Quota);
        }
        let result = if segment.complete {
            let next = version.checked_add(1).ok_or(MessagingError::Quota)?;
            sqlx::query("INSERT INTO contact_archives(owner,version,format,document) VALUES(?,?,1,?) ON CONFLICT(owner) DO UPDATE SET version=excluded.version,document=excluded.document")
                .bind(i64::from(key.user.get())).bind(next).bind(document).execute(&mut *tx).await.map_err(db)?;
            sqlx::query("DELETE FROM contact_staging WHERE owner=?")
                .bind(i64::from(key.user.get()))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            ArchiveResult::Published(next)
        } else {
            sqlx::query("UPDATE contact_staging SET document=? WHERE owner=?")
                .bind(document)
                .bind(i64::from(key.user.get()))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            ArchiveResult::Staged
        };
        #[cfg(feature = "test-hooks")]
        if crash_marker && std::env::var("RGG_TEST_CRASH").as_deref() == Ok("before_archive_commit")
        {
            std::process::exit(86);
        }
        tx.commit().await.map_err(db)?;
        #[cfg(feature = "test-hooks")]
        if crash_marker && std::env::var("RGG_TEST_CRASH").as_deref() == Ok("after_archive_commit")
        {
            std::process::exit(86);
        }
        Ok(result)
    }
    async fn export_archive(&self, key: &SessionKey) -> Result<ContactArchive, MessagingError> {
        let mut tx = self.writer.begin().await.map_err(db)?;
        fence(&mut tx, key, true).await?;
        let row = sqlx::query("SELECT version,document FROM contact_archives WHERE owner=?")
            .bind(i64::from(key.user.get()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
        let out = row
            .map(|r| ContactArchive {
                version: r.get("version"),
                text: r.get("document"),
            })
            .unwrap_or(ContactArchive {
                version: 0,
                text: String::new(),
            });
        validate_archive(&out.text).map_err(|_| MessagingError::Integrity)?;
        tx.commit().await.map_err(db)?;
        Ok(out)
    }
    async fn abort_archive(&self, key: &SessionKey) -> Result<(), MessagingError> {
        let mut tx = self.writer.begin().await.map_err(db)?;
        fence(&mut tx, key, false).await?;
        sqlx::query("DELETE FROM contact_staging WHERE owner=? AND session_id=?")
            .bind(i64::from(key.user.get()))
            .bind(key.id.as_slice())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }
}
