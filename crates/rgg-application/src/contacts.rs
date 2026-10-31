// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::messaging::{MessagingError, SessionKey};
use async_trait::async_trait;
pub const MAX_ARCHIVE_CHARACTERS: usize = 262144;
#[derive(Clone)]
pub struct ArchiveSegment {
    pub start: bool,
    pub complete: bool,
    pub text: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ArchiveResult {
    Staged,
    Published(i64),
}
pub struct ContactArchive {
    pub version: i64,
    pub text: String,
}
pub fn validate_archive(text: &str) -> Result<(), MessagingError> {
    if text.contains('\0')
        || text.len() > 3 * MAX_ARCHIVE_CHARACTERS
        || text.chars().count() > MAX_ARCHIVE_CHARACTERS
        || text.lines().count() > 4000
    {
        Err(MessagingError::Quota)
    } else {
        Ok(())
    }
}
#[async_trait]
pub trait ContactArchiveRepository: Send + Sync {
    async fn stage_archive(
        &self,
        key: &SessionKey,
        segment: ArchiveSegment,
    ) -> Result<ArchiveResult, MessagingError>;
    async fn export_archive(&self, key: &SessionKey) -> Result<ContactArchive, MessagingError>;
    async fn abort_archive(&self, key: &SessionKey) -> Result<(), MessagingError>;
}
