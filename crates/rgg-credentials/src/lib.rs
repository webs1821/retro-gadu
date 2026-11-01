// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Versioned credential envelopes. Keyring files are managed only while holding the data-directory lock.
pub mod gg_retro;
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305,
    aead::{Aead, Payload},
};
use rand::TryRngCore;
use rgg_application::accounts::{LegacyPassword, SealedSecret, SecretError, SecretProtector};
use rgg_domain::UserId;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
};
use zeroize::Zeroizing;

pub struct Keyring {
    active: String,
    keys: BTreeMap<String, Zeroizing<[u8; 32]>>,
}

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("keyring I/O failed")]
    Io(#[from] std::io::Error),
    #[error("keyring format is invalid")]
    Invalid,
    #[error("secure random source unavailable")]
    Random,
    #[error("keyring permissions must restrict access to its owner")]
    Permissions,
    #[error("keyring has reached its retained-key limit")]
    Full,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyFile {
    format: u32,
    active: String,
    keys: BTreeMap<String, Zeroizing<String>>,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

impl Keyring {
    pub fn load(path: &Path) -> Result<Self, KeyringError> {
        if !std::fs::symlink_metadata(path)?.file_type().is_file() {
            return Err(KeyringError::Invalid);
        }
        let file = std::fs::File::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if file.metadata()?.permissions().mode() & 0o077 != 0 {
                return Err(KeyringError::Permissions);
            }
        }
        let mut raw = Zeroizing::new(String::new());
        file.take(16 * 1024 + 1).read_to_string(&mut raw)?;
        if raw.len() > 16 * 1024 {
            return Err(KeyringError::Invalid);
        }
        let parsed: KeyFile = toml::from_str(&raw).map_err(|_| KeyringError::Invalid)?;
        if parsed.format != 1
            || !valid_id(&parsed.active)
            || parsed.keys.is_empty()
            || parsed.keys.len() > 32
        {
            return Err(KeyringError::Invalid);
        }
        let mut keys = BTreeMap::new();
        for (id, encoded) in parsed.keys {
            if !valid_id(&id) {
                return Err(KeyringError::Invalid);
            }
            let mut bytes = Zeroizing::new([0; 32]);
            hex::decode_to_slice(encoded.as_bytes(), &mut *bytes)
                .map_err(|_| KeyringError::Invalid)?;
            keys.insert(id, bytes);
        }
        if !keys.contains_key(&parsed.active) {
            return Err(KeyringError::Invalid);
        }
        Ok(Self {
            active: parsed.active,
            keys,
        })
    }

    /// Never overwrite or silently regenerate a missing keyring for an existing account database.
    pub fn create_new(path: &Path) -> Result<Self, KeyringError> {
        let mut key = Zeroizing::new([0u8; 32]);
        rand::rngs::OsRng
            .try_fill_bytes(&mut *key)
            .map_err(|_| KeyringError::Random)?;
        let mut identifier = [0u8; 12];
        rand::rngs::OsRng
            .try_fill_bytes(&mut identifier)
            .map_err(|_| KeyringError::Random)?;
        let active = format!("k{}", hex::encode(identifier));
        let encoded = Zeroizing::new(hex::encode(key.as_slice()));
        let text = Zeroizing::new(format!(
            "format = 1\nactive = \"{active}\"\n[keys]\n{active} = \"{}\"\n",
            encoded.as_str()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if file.metadata()?.permissions().mode() & 0o077 != 0 {
                return Err(KeyringError::Permissions);
            }
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            if let Some(parent) = path.parent() {
                std::fs::File::open(parent)?.sync_all()?;
            }
        }
        let mut keys = BTreeMap::new();
        keys.insert(active.clone(), key);
        Ok(Self { active, keys })
    }

    pub fn has_key(&self, id: &str) -> bool {
        self.keys.contains_key(id)
    }

    pub fn active_id(&self) -> &str {
        &self.active
    }
    pub fn key_ids(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    /// Retains every old read key. No pruning API exists because backups may still require them.
    pub fn successor(&self) -> Result<Self, KeyringError> {
        if self.keys.len() >= 32 {
            return Err(KeyringError::Full);
        }
        let mut key = Zeroizing::new([0u8; 32]);
        rand::rngs::OsRng
            .try_fill_bytes(&mut *key)
            .map_err(|_| KeyringError::Random)?;
        let mut id = [0u8; 12];
        rand::rngs::OsRng
            .try_fill_bytes(&mut id)
            .map_err(|_| KeyringError::Random)?;
        let active = format!("k{}", hex::encode(id));
        if self.keys.contains_key(&active) {
            return Err(KeyringError::Random);
        }
        let mut keys = self.keys.clone();
        keys.insert(active.clone(), key);
        Ok(Self { active, keys })
    }
    pub fn is_successor_of(&self, previous: &Self) -> bool {
        self.keys.len() == previous.keys.len() + 1
            && !previous.keys.contains_key(&self.active)
            && previous
                .keys
                .iter()
                .all(|(id, key)| self.keys.get(id) == Some(key))
    }
    /// Writes a fresh private candidate. Caller must publish it and sync the directory before rekeying rows.
    pub fn write_new(&self, path: &Path) -> Result<(), KeyringError> {
        // Reserve the full bounded representation before inserting secrets: growing a String
        // could otherwise free an earlier allocation without clearing its key material.
        let mut text = Zeroizing::new(String::with_capacity(16384));
        text.push_str(&format!(
            "format = 1\nactive = \"{}\"\n[keys]\n",
            self.active
        ));
        for (id, key) in &self.keys {
            let encoded = Zeroizing::new(hex::encode(key.as_slice()));
            text.push_str(id);
            text.push_str(" = \"");
            text.push_str(&encoded);
            text.push_str("\"\n");
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    }

    fn aad(user: UserId, version: u8, key_id: &str) -> Vec<u8> {
        let mut aad = b"rGG\0legacy-password\0".to_vec();
        aad.push(version);
        aad.extend_from_slice(&user.get().to_be_bytes());
        aad.push(key_id.len() as u8);
        aad.extend_from_slice(key_id.as_bytes());
        aad
    }
}

impl SecretProtector for Keyring {
    fn seal(&self, user: UserId, password: &LegacyPassword) -> Result<SealedSecret, SecretError> {
        let key = self
            .keys
            .get(&self.active)
            .ok_or(SecretError::KeyUnavailable)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(&**key).map_err(|_| SecretError::KeyUnavailable)?;
        let mut nonce = [0; 24];
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| SecretError::Random)?;
        let aad = Self::aad(user, 1, &self.active);
        let ciphertext = cipher
            .encrypt(
                &nonce.into(),
                Payload {
                    msg: password.expose(),
                    aad: &aad,
                },
            )
            .map_err(|_| SecretError::Integrity)?;
        Ok(SealedSecret {
            version: 1,
            key_id: self.active.clone(),
            nonce,
            ciphertext,
        })
    }

    fn open(&self, user: UserId, envelope: &SealedSecret) -> Result<LegacyPassword, SecretError> {
        if envelope.version != 1
            || !valid_id(&envelope.key_id)
            || !(24..=31).contains(&envelope.ciphertext.len())
        {
            return Err(SecretError::Integrity);
        }
        let key = self
            .keys
            .get(&envelope.key_id)
            .ok_or(SecretError::KeyUnavailable)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(&**key).map_err(|_| SecretError::KeyUnavailable)?;
        let aad = Self::aad(user, envelope.version, &envelope.key_id);
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    &envelope.nonce.into(),
                    Payload {
                        msg: &envelope.ciphertext,
                        aad: &aad,
                    },
                )
                .map_err(|_| SecretError::Integrity)?,
        );
        LegacyPassword::new(plaintext).map_err(|_| SecretError::Integrity)
    }
}

pub struct InvitationCodes;
impl rgg_application::registration::InvitationDigester for InvitationCodes {
    fn digest(&self, code: &str) -> Option<[u8; 32]> {
        invitation_digest(code)
    }
    fn legacy_digest(&self, code: &str) -> Option<[u8; 32]> {
        legacy_invitation_digest(code)
    }
}
/// Data-format compatibility with the pinned gg-retro fork; old codes are case sensitive.
pub fn legacy_invitation_digest(code: &str) -> Option<[u8; 32]> {
    if !(24..=128).contains(&code.len())
        || !code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return None;
    }
    Some(Sha256::digest(code.as_bytes()).into())
}
pub fn invitation_digest(code: &str) -> Option<[u8; 32]> {
    if code.len() != 32 || !code.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"rGG\0invitation\0");
    let normalized = Zeroizing::new(code.to_ascii_lowercase());
    hasher.update(normalized.as_bytes());
    Some(hasher.finalize().into())
}

pub fn new_invitation() -> Result<Zeroizing<String>, SecretError> {
    let mut bytes = Zeroizing::new([0; 16]);
    rand::rngs::OsRng
        .try_fill_bytes(&mut *bytes)
        .map_err(|_| SecretError::Random)?;
    Ok(Zeroizing::new(hex::encode(bytes.as_slice())))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user(value: u32) -> UserId {
        UserId::new(value).unwrap()
    }
    fn password() -> LegacyPassword {
        LegacyPassword::new(Zeroizing::new(b"LocalTest9!".to_vec())).unwrap()
    }

    #[test]
    fn envelopes_are_randomized_and_bound_to_account_version_nonce_and_key_id() {
        let root = tempfile::tempdir().unwrap();
        let ring = Keyring::create_new(&root.path().join("keys.toml")).unwrap();
        let sealed = ring.seal(user(112233), &password()).unwrap();
        let other = ring.seal(user(112233), &password()).unwrap();
        assert_ne!(sealed.nonce, other.nonce);
        assert_ne!(sealed.ciphertext, other.ciphertext);
        assert_eq!(
            ring.open(user(112233), &sealed).unwrap().expose(),
            password().expose()
        );
        assert!(ring.open(user(112234), &sealed).is_err());
        for field in 0..4 {
            let mut bad = sealed.clone();
            match field {
                0 => bad.version = 2,
                1 => bad.nonce[0] ^= 1,
                2 => bad.ciphertext[0] ^= 1,
                _ => bad.key_id = "missing".into(),
            }
            assert!(ring.open(user(112233), &bad).is_err());
        }
        assert!(!format!("{sealed:?}").contains(&hex::encode(&sealed.ciphertext)));
    }

    #[test]
    fn old_keys_are_readable_but_only_active_key_is_used_for_writing() {
        let root = tempfile::tempdir().unwrap();
        let mut ring = Keyring::create_new(&root.path().join("keys.toml")).unwrap();
        let old = ring.seal(user(112233), &password()).unwrap();
        let old_id = ring.active.clone();
        ring.keys.insert("key2".into(), Zeroizing::new([7; 32]));
        ring.active = "key2".into();
        let new = ring
            .seal(user(112233), &ring.open(user(112233), &old).unwrap())
            .unwrap();
        assert_eq!(new.key_id, "key2");
        ring.keys.remove(&old_id);
        assert!(matches!(
            ring.open(user(112233), &old),
            Err(SecretError::KeyUnavailable)
        ));
        assert_eq!(
            ring.open(user(112233), &new).unwrap().expose(),
            password().expose()
        );
    }

    #[test]
    fn key_files_survive_reload_refuse_overwrite_and_reject_invalid_data() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("keys.toml");
        let ring = Keyring::create_new(&path).unwrap();
        let sealed = ring.seal(user(112233), &password()).unwrap();
        assert!(Keyring::create_new(&path).is_err());
        assert_eq!(
            Keyring::load(&path)
                .unwrap()
                .open(user(112233), &sealed)
                .unwrap()
                .expose(),
            password().expose()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::write(&path, "format=1\nactive='missing'\n[keys]\n").unwrap();
        assert!(Keyring::load(&path).is_err());
    }

    #[test]
    fn invitation_format_is_unambiguous_and_case_insensitive() {
        let first = new_invitation().unwrap();
        let second = new_invitation().unwrap();
        assert_ne!(*first, *second);
        assert_eq!(
            invitation_digest(&first),
            invitation_digest(&first.to_uppercase())
        );
        assert!(invitation_digest("wrong").is_none());
    }

    #[test]
    fn legacy_invites_have_a_separate_case_sensitive_digest_and_strict_ascii_bounds() {
        let code = "Legacy-Invitation_Code_12345";
        assert_ne!(
            legacy_invitation_digest(code),
            legacy_invitation_digest(&code.to_lowercase())
        );
        assert!(invitation_digest(code).is_none());
        for length in [24, 128] {
            assert!(legacy_invitation_digest(&"A".repeat(length)).is_some());
        }
        for bad in [
            "A".repeat(23),
            "A".repeat(129),
            format!("{}!", "A".repeat(23)),
            "Ż".repeat(24),
        ] {
            assert!(legacy_invitation_digest(&bad).is_none());
        }
        let hex = "1234567890abcdef1234567890abcdef";
        assert_ne!(invitation_digest(hex), legacy_invitation_digest(hex));
    }

    #[test]
    fn successor_persists_all_old_keys_refuses_overwrite_and_stops_at_retention_limit() {
        let root = tempfile::tempdir().unwrap();
        let mut ring = Keyring::create_new(&root.path().join("original")).unwrap();
        let old = ring.seal(user(1), &password()).unwrap();
        for index in 1..32 {
            let mut next = ring.successor().unwrap();
            assert!(next.is_successor_of(&ring));
            let active = next.active.clone();
            next.active = ring.active.clone();
            assert!(!next.is_successor_of(&ring));
            next.active = active;
            let path = root.path().join(format!("next-{index}"));
            next.write_new(&path).unwrap();
            assert!(next.write_new(&path).is_err());
            ring = Keyring::load(&path).unwrap();
            assert_eq!(
                ring.open(user(1), &old).unwrap().expose(),
                password().expose()
            );
        }
        assert!(matches!(ring.successor(), Err(KeyringError::Full)));
        assert!(
            Keyring::load(&root.path().join("original"))
                .unwrap()
                .open(user(1), &old)
                .is_ok()
        );
    }
}
