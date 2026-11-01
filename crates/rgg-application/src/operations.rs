// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Private operator aggregates. These must never feed anonymous public statistics.
use crate::ports::StorageUnavailable;
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeliveryRecords {
    pub pending: u64,
    pub leased: u64,
    pub written_unconfirmed: u64,
    pub expired: u64,
    pub privacy_rejected: u64,
    pub rejected: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OperationalSnapshot {
    pub schema_version: u64,
    pub sampled_at: i64,
    pub active_accounts: u64,
    pub disabled_accounts: u64,
    pub retired_numbers: u64,
    /// Current valid sessions, including invisible and friends-only users.
    pub ready_sessions: u64,
    pub synchronizing_sessions: u64,
    /// Retained records, not cumulative delivery counters or recipient acknowledgements.
    pub deliveries: DeliveryRecords,
    pub queued_payload_bytes: u64,
    pub oldest_queue_age_seconds: u64,
    pub contact_archive_bytes: u64,
    /// Expiry only; current privacy and sessions still decide authorization.
    pub unexpired_image_grants: u64,
}
#[async_trait]
pub trait OperationalRepository: Send + Sync {
    async fn operational_snapshot(&self) -> Result<OperationalSnapshot, StorageUnavailable>;
}
