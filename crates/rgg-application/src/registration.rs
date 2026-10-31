// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::accounts::{
    AccountError, AccountRepository, LegacyPassword, Registration, SecretProtector,
};
use rgg_domain::UserId;
use std::sync::Arc;
use zeroize::Zeroizing;

pub trait InvitationDigester: Send + Sync {
    fn digest(&self, code: &str) -> Option<[u8; 32]>;
}
pub struct NewAccount {
    pub requested_uin: Option<UserId>,
    pub invitation: Zeroizing<String>,
    pub password: LegacyPassword,
    pub email: String,
}
pub struct RegistrationService {
    accounts: Arc<dyn AccountRepository>,
    protector: Arc<dyn SecretProtector>,
    invitations: Arc<dyn InvitationDigester>,
}
impl RegistrationService {
    pub fn new(
        accounts: Arc<dyn AccountRepository>,
        protector: Arc<dyn SecretProtector>,
        invitations: Arc<dyn InvitationDigester>,
    ) -> Self {
        Self {
            accounts,
            protector,
            invitations,
        }
    }
    pub async fn register(&self, input: NewAccount) -> Result<UserId, AccountError> {
        let digest = self
            .invitations
            .digest(input.invitation.trim())
            .ok_or(AccountError::InvalidInvite)?;
        let registration = Registration {
            requested_uin: input.requested_uin,
            invite_digest: digest,
            password: input.password,
            email: input.email,
        };
        registration.validate()?;
        // New-account allocation has no existing actor/session to revoke. The repository
        // owns the same atomic invitation/allocation/encryption transaction used by CLI.
        self.accounts
            .register_with_invite(registration, &*self.protector)
            .await
    }
}
