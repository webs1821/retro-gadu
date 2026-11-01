// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SCHEMA_VERSION, SqliteStore, database as sqlx};
use async_trait::async_trait;
use rgg_application::{
    operations::{OperationalRepository, OperationalSnapshot},
    ports::StorageUnavailable,
};
use sqlx::Row;

fn number(value: i64) -> Result<u64, StorageUnavailable> {
    u64::try_from(value).map_err(|_| StorageUnavailable)
}

#[async_trait]
impl OperationalRepository for SqliteStore {
    async fn operational_snapshot(&self) -> Result<OperationalSnapshot, StorageUnavailable> {
        // One bounded read snapshot. Never materialize identifiers, descriptions or payloads.
        let mut tx = self.pool.begin().await.map_err(|_| StorageUnavailable)?;
        let row = sqlx::query("SELECT unixepoch() sampled_at,coalesce(sum(active=1),0) active,coalesce(sum(active=0),0) disabled FROM accounts")
            .fetch_one(&mut *tx).await.map_err(|_| StorageUnavailable)?;
        let mut result = OperationalSnapshot {
            schema_version: SCHEMA_VERSION as u64,
            sampled_at: row.get("sampled_at"),
            active_accounts: number(row.get("active"))?,
            disabled_accounts: number(row.get("disabled"))?,
            ..OperationalSnapshot::default()
        };
        result.retired_numbers = number(
            sqlx::query_scalar("SELECT count(*) FROM retired_uins")
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| StorageUnavailable)?,
        )?;
        let row = sqlx::query("SELECT coalesce(sum(s.ready=1),0) ready,coalesce(sum(s.ready=0),0) synchronizing FROM account_sessions s JOIN accounts a ON a.uin=s.uin JOIN runtime_state r ON r.id=1 WHERE s.epoch=r.process_epoch AND a.active=1 AND a.credential_version=s.credential_version AND a.session_generation=s.generation AND (s.ready=0 OR EXISTS(SELECT 1 FROM account_presence p WHERE p.uin=s.uin AND p.session_id=s.session_id))")
            .fetch_one(&mut *tx).await.map_err(|_| StorageUnavailable)?;
        result.ready_sessions = number(row.get("ready"))?;
        result.synchronizing_sessions = number(row.get("synchronizing"))?;
        let rows = sqlx::query("SELECT d.state,count(*) records,coalesce(sum(m.payload_bytes),0) bytes,min(m.accepted_at) oldest FROM deliveries d JOIN messages m ON m.id=d.message_id GROUP BY d.state")
            .fetch_all(&mut *tx).await.map_err(|_| StorageUnavailable)?;
        for row in rows {
            let state: String = row.get("state");
            let count = number(row.get("records"))?;
            let target = match state.as_str() {
                "Pending" => &mut result.deliveries.pending,
                "Leased" => &mut result.deliveries.leased,
                "TransportWrittenUnconfirmed" => &mut result.deliveries.written_unconfirmed,
                "Expired" => &mut result.deliveries.expired,
                "RejectedByPrivacy" => &mut result.deliveries.privacy_rejected,
                "Rejected" => &mut result.deliveries.rejected,
                _ => return Err(StorageUnavailable),
            };
            *target = count;
            if matches!(state.as_str(), "Pending" | "Leased") {
                result.queued_payload_bytes = result
                    .queued_payload_bytes
                    .checked_add(number(row.get("bytes"))?)
                    .ok_or(StorageUnavailable)?;
                let oldest: i64 = row.get("oldest");
                result.oldest_queue_age_seconds = result
                    .oldest_queue_age_seconds
                    .max(result.sampled_at.saturating_sub(oldest).max(0) as u64);
            }
        }
        result.contact_archive_bytes = number(
            sqlx::query_scalar(
                "SELECT coalesce(sum(length(CAST(document AS BLOB))),0) FROM contact_archives",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| StorageUnavailable)?,
        )?;
        result.unexpired_image_grants = number(
            sqlx::query_scalar("SELECT count(*) FROM image_offers WHERE expires_at>?")
                .bind(result.sampled_at)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| StorageUnavailable)?,
        )?;
        tx.commit().await.map_err(|_| StorageUnavailable)?;
        Ok(result)
    }
}
