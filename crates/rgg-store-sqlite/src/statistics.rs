// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use rgg_application::{
    ports::StorageUnavailable,
    presence::{Availability, Presence, publicly_online},
    statistics::{PublicStatistics, StatisticsRepository},
};
use sqlx::Row;

#[async_trait]
impl StatisticsRepository for SqliteStore {
    async fn public_statistics(&self) -> Result<PublicStatistics, StorageUnavailable> {
        let mut tx = self.pool.begin().await.map_err(|_| StorageUnavailable)?;
        let row = sqlx::query("SELECT count(*) registered,unixepoch() sampled_at FROM accounts")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| StorageUnavailable)?;
        let registered =
            u64::try_from(row.get::<i64, _>("registered")).map_err(|_| StorageUnavailable)?;
        let sampled_at = row.get("sampled_at");
        let mut online = 0;
        {
            // Every join is fail-closed. Never load identifiers, descriptions, emails or secrets.
            // One read transaction keeps both counters consistent with concurrent registration.
            let mut rows=sqlx::query("SELECT p.availability,priv.friends_only FROM account_sessions s JOIN accounts a ON a.uin=s.uin JOIN account_presence p ON p.uin=s.uin AND p.session_id=s.session_id JOIN account_privacy priv ON priv.uin=s.uin JOIN runtime_state r ON r.id=1 WHERE s.ready=1 AND s.epoch=r.process_epoch AND a.active=1 AND a.credential_version=s.credential_version AND a.session_generation=s.generation").fetch(&mut *tx);
            while let Some(row) = rows.try_next().await.map_err(|_| StorageUnavailable)? {
                let availability = match row.get::<String, _>("availability").as_str() {
                    "available" => Availability::Available,
                    "away" => Availability::Away,
                    "offline" => Availability::Offline,
                    "invisible" => Availability::Invisible,
                    _ => return Err(StorageUnavailable),
                };
                let presence = Presence {
                    availability,
                    description: None,
                    return_at: None,
                    friends_only: row.get::<i64, _>("friends_only") != 0,
                };
                online += u64::from(publicly_online(&presence));
            }
        }
        tx.commit().await.map_err(|_| StorageUnavailable)?;
        Ok(PublicStatistics {
            registered,
            publicly_online: online,
            sampled_at,
        })
    }
}
