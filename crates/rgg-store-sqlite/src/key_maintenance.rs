// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Offline re-encryption under the data-directory lock; never changes a user's password.
use crate::{SqliteStore, database as sqlx};
use futures_util::TryStreamExt;
use rgg_application::accounts::{SealedSecret, SecretProtector};
use rgg_domain::UserId;
use sqlx::Row;
use std::collections::BTreeMap;
type Failure = &'static str;
fn decode(row: &sqlx::sqlite::SqliteRow) -> Result<(UserId, SealedSecret), Failure> {
    let number: i64 = row.try_get("uin").map_err(|_| "invalid credential row")?;
    let user = UserId::new(number.try_into().map_err(|_| "invalid credential row")?)
        .map_err(|_| "invalid credential row")?;
    let version: i64 = row
        .try_get("secret_version")
        .map_err(|_| "invalid credential row")?;
    Ok((
        user,
        SealedSecret {
            version: version.try_into().map_err(|_| "invalid credential row")?,
            key_id: row
                .try_get("key_id")
                .map_err(|_| "invalid credential row")?,
            nonce: row
                .try_get::<Vec<u8>, _>("nonce")
                .map_err(|_| "invalid credential row")?
                .try_into()
                .map_err(|_| "invalid credential row")?,
            ciphertext: row
                .try_get("ciphertext")
                .map_err(|_| "invalid credential row")?,
        },
    ))
}

impl SqliteStore {
    pub async fn inspect_credentials(
        &self,
        protector: &dyn SecretProtector,
    ) -> Result<BTreeMap<String, u64>, Failure> {
        let invalid:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE length(key_id)>32 OR length(nonce)!=24 OR length(ciphertext) NOT BETWEEN 24 AND 31 OR secret_version!=1)").fetch_one(&self.pool).await.map_err(|_|"credential inspection failed")?;
        if invalid != 0 {
            return Err("invalid credential row");
        }
        let mut counts = BTreeMap::new();
        let mut rows = sqlx::query(
            "SELECT uin,secret_version,key_id,nonce,ciphertext FROM accounts ORDER BY uin",
        )
        .fetch(&self.pool);
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|_| "credential inspection failed")?
        {
            let (user, secret) = decode(&row)?;
            protector
                .open(user, &secret)
                .map_err(|_| "keyring cannot decrypt every account")?;
            *counts.entry(secret.key_id).or_insert(0) += 1;
        }
        Ok(counts)
    }
    /// Publishes one bounded transaction. A new nonce reservation and the row update are atomic.
    /// The supplied protector's active key must already have been durably published to keys.toml.
    pub async fn rekey_batch(
        &self,
        protector: &dyn SecretProtector,
        active: &str,
        limit: u32,
    ) -> Result<u64, Failure> {
        if !(1..=64).contains(&limit) {
            return Err("invalid rekey batch size");
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| "rekey transaction failed")?;
        let rows=sqlx::query("SELECT uin,secret_version,key_id,nonce,ciphertext FROM accounts WHERE key_id!=? ORDER BY uin LIMIT ?")
            .bind(active).bind(limit).fetch_all(&mut *tx).await.map_err(|_|"rekey query failed")?;
        for row in &rows {
            let (user, old) = decode(row)?;
            let password = protector
                .open(user, &old)
                .map_err(|_| "rekey decryption failed")?;
            let new = protector
                .seal(user, &password)
                .map_err(|_| "rekey encryption failed")?;
            drop(password);
            if new.key_id != active {
                return Err("rekey protector active key mismatch");
            }
            sqlx::query("INSERT INTO secret_nonces(key_id,nonce) VALUES(?,?)")
                .bind(&new.key_id)
                .bind(new.nonce.as_slice())
                .execute(&mut *tx)
                .await
                .map_err(|_| "rekey nonce reservation failed")?;
            let changed=sqlx::query("UPDATE accounts SET secret_version=?,key_id=?,nonce=?,ciphertext=? WHERE uin=? AND key_id=? AND nonce=? AND ciphertext=?")
                .bind(i64::from(new.version)).bind(new.key_id).bind(new.nonce.as_slice()).bind(new.ciphertext).bind(i64::from(user.get())).bind(old.key_id).bind(old.nonce.as_slice()).bind(old.ciphertext)
                .execute(&mut *tx).await.map_err(|_|"rekey update failed")?.rows_affected();
            if changed != 1 {
                return Err("credential changed during offline rekey");
            }
        }
        tx.commit().await.map_err(|_| "rekey commit failed")?;
        Ok(rows.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::accounts::{AccountRepository, LegacyPassword, Registration, SecretError};
    use rgg_credentials::Keyring;
    use zeroize::Zeroizing;
    struct ReusedNonce {
        old: Keyring,
        first: SealedSecret,
    }
    impl SecretProtector for ReusedNonce {
        fn open(&self, u: UserId, s: &SealedSecret) -> Result<LegacyPassword, SecretError> {
            self.old.open(u, s)
        }
        fn seal(&self, _: UserId, _: &LegacyPassword) -> Result<SealedSecret, SecretError> {
            Ok(self.first.clone())
        }
    }
    #[tokio::test]
    async fn reused_nonce_rolls_back_the_entire_batch_then_resume_succeeds() {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(root.path()).await.unwrap();
        let ring = Keyring::create_new(&root.path().join("keys.toml")).unwrap();
        store.create_invite([7; 32], 2).await.unwrap();
        let password = || LegacyPassword::new(Zeroizing::new(b"LocalTest9!".to_vec())).unwrap();
        for n in [1, 2] {
            store
                .register_with_invite(
                    Registration {
                        requested_uin: Some(UserId::new(n).unwrap()),
                        invite_digest: Some([7; 32]),
                        legacy_invite_digest: None,
                        password: password(),
                        email: "synthetic@example.invalid".into(),
                    },
                    &ring,
                )
                .await
                .unwrap();
        }
        let next = ring.successor().unwrap();
        let first = next.seal(UserId::new(1).unwrap(), &password()).unwrap();
        let old_id = ring.active_id().to_string();
        let broken = ReusedNonce { old: ring, first };
        assert_eq!(
            store
                .rekey_batch(&broken, next.active_id(), 64)
                .await
                .unwrap_err(),
            "rekey nonce reservation failed"
        );
        assert_eq!(
            store
                .inspect_credentials(&broken)
                .await
                .unwrap()
                .get(&old_id),
            Some(&2)
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM secret_nonces")
                .fetch_one(&store.pool)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .rekey_batch(&next, next.active_id(), 64)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .rekey_batch(&next, next.active_id(), 64)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .inspect_credentials(&next)
                .await
                .unwrap()
                .get(next.active_id()),
            Some(&2)
        );
        store.close().await;
    }
}
