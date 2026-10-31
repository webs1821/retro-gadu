// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::ports::StorageUnavailable;
use async_trait::async_trait;

/// Anonymous aggregate from one database snapshot, not a session/user list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicStatistics {
    /// Existing accounts, including disabled accounts; excludes retired tombstones.
    pub registered: u64,
    /// Ready, current sessions whose presence is visible to an anonymous stranger.
    pub publicly_online: u64,
    pub sampled_at: i64,
}
#[async_trait]
pub trait StatisticsRepository: Send + Sync {
    async fn public_statistics(&self) -> Result<PublicStatistics, StorageUnavailable>;
}
