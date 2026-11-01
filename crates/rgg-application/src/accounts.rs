// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use async_trait::async_trait;
use rgg_domain::UserId;
use zeroize::Zeroizing;

pub struct LegacyPassword(Zeroizing<Vec<u8>>);
impl std::fmt::Debug for LegacyPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LegacyPassword([REDACTED])")
    }
}
impl LegacyPassword {
    /// Initial registration policy matches the existing gg-retro deployment.
    /// Restricting to printable ASCII avoids silent password transcoding by old clients.
    pub fn new(bytes: Zeroizing<Vec<u8>>) -> Result<Self, AccountError> {
        if !(8..=15).contains(&bytes.len()) || !bytes.iter().all(|byte| (33..=126).contains(byte)) {
            return Err(AccountError::InvalidPassword);
        }
        Ok(Self(bytes))
    }
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
    /// Compare the complete password in a fixed-size padded buffer, not its GG32 response.
    pub fn matches(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        let mut left = Zeroizing::new([0u8; 16]);
        let mut right = Zeroizing::new([0u8; 16]);
        left[..self.0.len()].copy_from_slice(&self.0);
        right[..other.0.len()].copy_from_slice(&other.0);
        left[15] = self.0.len() as u8;
        right[15] = other.0.len() as u8;
        bool::from(left.as_slice().ct_eq(right.as_slice()))
    }
}

#[derive(Clone)]
pub struct SealedSecret {
    pub version: u8,
    pub key_id: String,
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}
impl std::fmt::Debug for SealedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SealedSecret([REDACTED])")
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SecretError {
    #[error("secret key unavailable")]
    KeyUnavailable,
    #[error("secret envelope failed verification")]
    Integrity,
    #[error("secure random source unavailable")]
    Random,
    #[error("secret nonce was already used with this key")]
    NonceReuse,
}

pub trait SecretProtector: Send + Sync {
    fn seal(&self, user: UserId, password: &LegacyPassword) -> Result<SealedSecret, SecretError>;
    fn open(&self, user: UserId, envelope: &SealedSecret) -> Result<LegacyPassword, SecretError>;
}

pub struct Registration {
    pub requested_uin: Option<UserId>,
    pub invite_digest: Option<[u8; 32]>,
    pub legacy_invite_digest: Option<[u8; 32]>,
    pub password: LegacyPassword,
    pub email: String,
}
impl Registration {
    pub fn validate(&self) -> Result<(), AccountError> {
        if self.invite_digest.is_none() && self.legacy_invite_digest.is_none() {
            return Err(AccountError::InvalidInvite);
        }
        if self
            .requested_uin
            .is_some_and(|user| user.get() > 0x00ff_ffff)
        {
            return Err(AccountError::UnsupportedNumber);
        }
        validate_email(&self.email)
    }
}

pub fn validate_email(email: &str) -> Result<(), AccountError> {
    if email.len() > 254
        || email.chars().any(|c| c.is_control() || c.is_whitespace())
        || email.split_once('@').is_none_or(|(local, domain)| {
            local.is_empty() || domain.is_empty() || domain.contains('@')
        })
    {
        return Err(AccountError::InvalidEmail);
    }
    Ok(())
}

pub struct LoginCredential {
    pub user: UserId,
    pub credential_version: u64,
    pub secret: SealedSecret,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AccountError {
    #[error("password must contain 8 to 15 printable ASCII characters without spaces")]
    InvalidPassword,
    #[error("invalid email address")]
    InvalidEmail,
    #[error("number is outside the currently supported GG6 range")]
    UnsupportedNumber,
    #[error("invitation is invalid or exhausted")]
    InvalidInvite,
    #[error("number is reserved or already in use")]
    NumberTaken,
    #[error("automatic number range is exhausted")]
    NumberRangeExhausted,
    #[error("account is missing or inactive")]
    Inactive,
    #[error("credential version is no longer current")]
    StaleCredential,
    #[error("current password is incorrect")]
    IncorrectPassword,
    #[error("new password must differ from the current password")]
    UnchangedPassword,
    #[error("offline password change refuses an account with an active session")]
    ActiveSession,
    #[error("account storage unavailable")]
    Storage,
    #[error(transparent)]
    Secret(#[from] SecretError),
}

#[async_trait]
pub trait AccountRepository: Send + Sync {
    async fn create_invite(&self, digest: [u8; 32], uses: u32) -> Result<(), AccountError>;
    /// Entire number allocation, invitation consumption and insert happen in one transaction.
    async fn register_with_invite(
        &self,
        input: Registration,
        protector: &dyn SecretProtector,
    ) -> Result<UserId, AccountError>;
    async fn login_credential(&self, user: UserId) -> Result<LoginCredential, AccountError>;
    async fn retire_account(&self, user: UserId) -> Result<(), AccountError>;
    async fn registered_count(&self) -> Result<u64, AccountError>;
    async fn required_key_ids(&self) -> Result<Vec<String>, AccountError>;
    async fn credential_key_probes(&self) -> Result<Vec<LoginCredential>, AccountError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_policy_rejects_silent_transcoding_and_redacts_debug() {
        let password = LegacyPassword::new(Zeroizing::new(b"LocalTest9!".to_vec())).unwrap();
        assert!(!format!("{password:?}").contains("LocalTest9!"));
        for value in [
            b"short".as_slice(),
            b"contains space",
            b"1234567890123456",
            b"1234567\x80",
            b"1234567\0",
        ] {
            assert!(LegacyPassword::new(Zeroizing::new(value.to_vec())).is_err());
        }
    }
}
