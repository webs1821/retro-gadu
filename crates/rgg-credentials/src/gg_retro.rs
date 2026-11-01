// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Independent reader of the pinned gg-retro data format, not a port of its vault code.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305,
    aead::{Aead, Payload},
};
use rgg_application::accounts::{LegacyPassword, SecretError};
use rgg_domain::UserId;
use std::{io::Read, path::Path};
use zeroize::Zeroizing;
pub struct LegacyKey(Zeroizing<[u8; 32]>);
impl LegacyKey {
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let meta = std::fs::symlink_metadata(path).map_err(|_| "legacy key unavailable")?;
        if !meta.file_type().is_file() || meta.len() != 32 {
            return Err("legacy key must be a regular file of exactly 32 bytes");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err("legacy key must be owner-only");
            }
        }
        let mut input = std::fs::File::open(path).map_err(|_| "legacy key unavailable")?;
        let mut data = Zeroizing::new([0u8; 32]);
        input
            .read_exact(&mut *data)
            .map_err(|_| "legacy key unavailable")?;
        let mut extra = [0];
        if input
            .read(&mut extra)
            .map_err(|_| "legacy key unavailable")?
            != 0
        {
            return Err("legacy key size changed");
        }
        Ok(Self(data))
    }
    pub fn open(&self, user: UserId, encoded: &str) -> Result<LegacyPassword, SecretError> {
        if encoded.len() > 512 {
            return Err(SecretError::Integrity);
        }
        let data = STANDARD
            .decode(
                encoded
                    .strip_prefix("ggv1:")
                    .ok_or(SecretError::Integrity)?,
            )
            .map_err(|_| SecretError::Integrity)?;
        if !(48..=55).contains(&data.len()) {
            return Err(SecretError::Integrity);
        }
        let nonce: [u8; 24] = data[..24].try_into().map_err(|_| SecretError::Integrity)?;
        let cipher = XChaCha20Poly1305::new_from_slice(self.0.as_slice())
            .map_err(|_| SecretError::KeyUnavailable)?;
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    &nonce.into(),
                    Payload {
                        msg: &data[24..],
                        aad: &user.get().to_le_bytes(),
                    },
                )
                .map_err(|_| SecretError::Integrity)?,
        );
        LegacyPassword::new(plaintext).map_err(|_| SecretError::Integrity)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_libsodium_vector_binds_key_account_and_ciphertext() {
        // libsodium.so.23 crypto_aead_xchacha20poly1305_ietf_encrypt; synthetic key/nonce 00..1f/00..17.
        let encoded = "ggv1:AAECAwQFBgcICQoLDA0ODxAREhMUFRYX0q1sHvyG6N1HfQfr+rRf6HNVSyA5XrPmuoi2";
        let key = LegacyKey(Zeroizing::new(std::array::from_fn(|i| i as u8)));
        let user = UserId::new(742).unwrap();
        assert_eq!(key.open(user, encoded).unwrap().expose(), b"LocalTest9!");
        assert!(key.open(UserId::new(743).unwrap(), encoded).is_err());
        assert!(
            LegacyKey(Zeroizing::new([99; 32]))
                .open(user, encoded)
                .is_err()
        );
        assert!(key.open(user, "LocalTest9!").is_err());
        assert!(
            key.open(user, &format!("ggv1:{}", "A".repeat(513)))
                .is_err()
        );
        let mut modified = encoded.to_string();
        modified.pop();
        modified.push('A');
        assert!(key.open(user, &modified).is_err());
    }
}
