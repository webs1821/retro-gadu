// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Full-password self-service; only authenticated encrypted commands reach the account actor.
use crate::{
    accounts::{AccountError, AccountRepository, LegacyPassword, SealedSecret, SecretProtector},
    sessions::SessionHub,
};
use async_trait::async_trait;
use rgg_domain::UserId;
use std::sync::Arc;

pub struct PasswordChange {
    pub user: UserId,
    pub current: LegacyPassword,
    pub replacement: LegacyPassword,
}
/// No public constructor or deserialization: this proof is made only after full authentication.
pub struct AuthorizedPasswordChange {
    user: UserId,
    version: i64,
    secret: SealedSecret,
}
impl AuthorizedPasswordChange {
    pub fn user(&self) -> UserId {
        self.user
    }
    pub fn expected_version(&self) -> i64 {
        self.version
    }
    pub fn secret(&self) -> &SealedSecret {
        &self.secret
    }
}
#[async_trait]
pub trait PasswordRepository: AccountRepository {
    /// Five attempts per existing active account/600s; unknown accounts allocate no state.
    async fn charge_password_attempt(&self, user: UserId, now: i64) -> Result<bool, AccountError>;
    async fn password_change_is_current(
        &self,
        change: &AuthorizedPasswordChange,
    ) -> Result<bool, AccountError>;
    /// Caller holds the publication write gate and has stopped the previous account socket.
    async fn commit_password_change(
        &self,
        change: &AuthorizedPasswordChange,
        now: i64,
    ) -> Result<i64, AccountError>;
}
pub struct PasswordService {
    repository: Arc<dyn PasswordRepository>,
    protector: Arc<dyn SecretProtector>,
    hub: Arc<SessionHub>,
    dummy: SealedSecret,
}
impl PasswordService {
    pub fn new(
        repository: Arc<dyn PasswordRepository>,
        protector: Arc<dyn SecretProtector>,
        hub: Arc<SessionHub>,
    ) -> Result<Self, AccountError> {
        let password = LegacyPassword::new(zeroize::Zeroizing::new(b"rGGdummy9!".to_vec()))?;
        let dummy = protector.seal(UserId::new(1).expect("nonzero"), &password)?;
        Ok(Self {
            repository,
            protector,
            hub,
            dummy,
        })
    }
    pub async fn change(&self, input: PasswordChange) -> Result<i64, AccountError> {
        let admitted = self
            .repository
            .charge_password_attempt(input.user, self.hub.now())
            .await?;
        let credential = match self.repository.login_credential(input.user).await {
            Ok(value) => Some(value),
            Err(AccountError::Inactive) => None,
            Err(error) => return Err(error),
        };
        let saved = match &credential {
            Some(value) => self.protector.open(input.user, &value.secret)?,
            None => self
                .protector
                .open(UserId::new(1).expect("nonzero"), &self.dummy)?,
        };
        let correct = saved.matches(&input.current);
        if !admitted || !correct || credential.is_none() {
            return Err(AccountError::IncorrectPassword);
        }
        if saved.matches(&input.replacement) {
            return Err(AccountError::UnchangedPassword);
        }
        drop(saved);
        let change = AuthorizedPasswordChange {
            user: input.user,
            version: credential
                .expect("authenticated")
                .credential_version
                .try_into()
                .map_err(|_| AccountError::StaleCredential)?,
            secret: self.protector.seal(input.user, &input.replacement)?,
        };
        drop(input);
        self.hub
            .change_password(change, self.repository.clone())
            .await
    }
}
