// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Authenticated commands, durable delivery contracts and storage-neutral data.
use crate::accounts::{AccountRepository, LegacyPassword, SealedSecret, SecretProtector};
use async_trait::async_trait;
use rgg_domain::{MessageText, UserId};
use serde::{Deserialize, Serialize};

pub trait Clock: Send + Sync {
    fn unix_seconds(&self) -> i64;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionKey {
    pub user: UserId,
    pub id: [u8; 16],
    pub generation: i64,
    pub credential_version: i64,
    pub epoch: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactRule {
    pub user: UserId,
    pub flags: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextStyle {
    pub position: u16,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Option<[u8; 3]>,
}

#[derive(Clone, Debug)]
pub struct MessageBundle {
    pub recipient: UserId,
    pub client_sequence: u32,
    /// Presentation bits only; delivery and acknowledgement flags belong to adapters.
    pub presentation: u8,
    pub text: MessageText,
    pub styles: Vec<TextStyle>,
}

#[derive(Clone, Debug)]
pub struct DeliveryLease {
    pub id: i64,
    pub token: [u8; 16],
    pub sender: UserId,
    pub client_sequence: u32,
    pub accepted_at: i64,
    pub presentation: u8,
    pub queued: bool,
    pub text: MessageText,
    pub styles: Vec<TextStyle>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueueLimits {
    pub per_recipient_messages: u32,
    pub per_sender_messages: u32,
    pub global_messages: u32,
    pub per_recipient_bytes: u32,
    pub global_bytes: u32,
    pub ttl_seconds: u32,
}
impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            per_recipient_messages: 1000,
            per_sender_messages: 1000,
            global_messages: 10000,
            per_recipient_bytes: 4 * 1024 * 1024,
            global_bytes: 32 * 1024 * 1024,
            ttl_seconds: 30 * 86400,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum MessagingError {
    #[error("session or credential is no longer current")]
    Stale,
    #[error("initial privacy synchronization is incomplete")]
    NotReady,
    #[error("recipient cannot receive this message")]
    Rejected,
    #[error("queue quota exceeded")]
    Quota,
    #[error("invalid command")]
    Invalid,
    #[error("storage unavailable")]
    Storage,
    #[error("stored payload failed integrity validation")]
    Integrity,
    #[error("previous transport has not stopped")]
    Busy,
}

/// Constructed only by successful application authentication, not by network deserialization.
pub struct VerifiedIdentity {
    pub(crate) user: UserId,
    pub(crate) credential_version: i64,
}
pub trait PasswordProof: Send + Sync {
    fn matches(&self, password: &[u8]) -> bool;
}

pub struct Authenticator {
    accounts: std::sync::Arc<dyn AccountRepository>,
    protector: std::sync::Arc<dyn SecretProtector>,
    dummy: SealedSecret,
}
impl Authenticator {
    pub fn new(
        accounts: std::sync::Arc<dyn AccountRepository>,
        protector: std::sync::Arc<dyn SecretProtector>,
    ) -> Result<Self, MessagingError> {
        let password = LegacyPassword::new(zeroize::Zeroizing::new(b"rGGdummy9!".to_vec()))
            .map_err(|_| MessagingError::Integrity)?;
        let dummy = protector
            .seal(UserId::new(1).expect("nonzero"), &password)
            .map_err(|_| MessagingError::Storage)?;
        Ok(Self {
            accounts,
            protector,
            dummy,
        })
    }
    pub async fn authenticate(
        &self,
        user: UserId,
        proof: &dyn PasswordProof,
    ) -> Result<VerifiedIdentity, MessagingError> {
        match self.accounts.login_credential(user).await {
            Ok(credential) => {
                let password = self
                    .protector
                    .open(user, &credential.secret)
                    .map_err(|_| MessagingError::Rejected)?;
                if !proof.matches(password.expose()) {
                    return Err(MessagingError::Rejected);
                }
                Ok(VerifiedIdentity {
                    user,
                    credential_version: credential
                        .credential_version
                        .try_into()
                        .map_err(|_| MessagingError::Integrity)?,
                })
            }
            Err(_) => {
                let password = self
                    .protector
                    .open(UserId::new(1).expect("nonzero"), &self.dummy)
                    .map_err(|_| MessagingError::Storage)?;
                let _ = proof.matches(password.expose());
                Err(MessagingError::Rejected)
            }
        }
    }
}

#[async_trait]
pub trait MessagingRepository:
    Send + Sync + crate::presence::PresenceRepository + crate::contacts::ContactArchiveRepository
{
    /// Caller has stopped the previous socket before this atomic attachment.
    async fn attach_session_if_current(
        &self,
        user: UserId,
        credential_version: i64,
        id: [u8; 16],
        now: i64,
    ) -> Result<SessionKey, MessagingError>;
    async fn session_is_current(&self, session: &SessionKey) -> Result<bool, MessagingError>;
    async fn detach_session(&self, session: &SessionKey, now: i64) -> Result<(), MessagingError>;
    async fn begin_initial_state(&self, session: &SessionKey) -> Result<(), MessagingError>;
    /// Socket writer is idle and has no outstanding lease when this is called.
    async fn replace_privacy(
        &self,
        session: &SessionKey,
        rules: &[ContactRule],
        friends_only: bool,
        now: i64,
    ) -> Result<(), MessagingError>;
    async fn change_contact(
        &self,
        session: &SessionKey,
        rule: ContactRule,
        remove: bool,
        now: i64,
    ) -> Result<(), MessagingError>;
    async fn change_friends_only(
        &self,
        session: &SessionKey,
        value: bool,
    ) -> Result<(), MessagingError>;
    async fn accept_message_bundle(
        &self,
        session: &SessionKey,
        message: MessageBundle,
        now: i64,
        limits: &QueueLimits,
    ) -> Result<i64, MessagingError>;
    async fn claim_delivery(
        &self,
        session: &SessionKey,
        now: i64,
    ) -> Result<Option<DeliveryLease>, MessagingError>;
    async fn record_transport_write(
        &self,
        session: &SessionKey,
        lease: &DeliveryLease,
        now: i64,
    ) -> Result<(), MessagingError>;
    /// Only after socket close or proof that the lease was never handed to a writer.
    async fn release_delivery(
        &self,
        session: &SessionKey,
        lease: &DeliveryLease,
        now: i64,
    ) -> Result<(), MessagingError>;
    async fn prune(&self, now: i64) -> Result<(), MessagingError>;
    /// Caller first revokes I/O and awaits the independent stopped barrier.
    async fn revoke_credentials(
        &self,
        user: UserId,
        version: i64,
        now: i64,
    ) -> Result<(), MessagingError>;
}
