// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::SqliteStore;
use crate::database as sqlx;
use async_trait::async_trait;
use rgg_application::accounts::{
    AccountError, AccountRepository, LoginCredential, Registration, SealedSecret, SecretProtector,
};
use rgg_domain::UserId;
use sqlx::{Row, Sqlite, Transaction};

fn database_error(_: sqlx::Error) -> AccountError {
    AccountError::Storage
}

async fn register(
    tx: &mut Transaction<'_, Sqlite>,
    input: Registration,
    protector: &dyn SecretProtector,
) -> Result<UserId, AccountError> {
    // First operation takes SQLite's write reservation. This prevents deferred read->write
    // upgrade races; invitation, number and account are still rolled back together on error.
    let consumed =
        sqlx::query("UPDATE invitations SET used=used+1 WHERE digest=? AND used<max_uses")
            .bind(input.invite_digest.as_ref().map(|v| v.as_slice()))
            .execute(&mut **tx)
            .await
            .map_err(database_error)?
            .rows_affected();
    if consumed != 1 {
        // An existing modern code remains authoritative even after exhaustion. Never fall
        // through to a legacy hash and silently give the same textual code extra uses.
        let modern_exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM invitations WHERE digest=?)")
                .bind(input.invite_digest.as_ref().map(|v| v.as_slice()))
                .fetch_one(&mut **tx)
                .await
                .map_err(database_error)?;
        if modern_exists {
            return Err(AccountError::InvalidInvite);
        }
        let legacy=sqlx::query("UPDATE legacy_invitations SET used=used+1 WHERE digest=? AND active=1 AND used<max_uses")
            .bind(input.legacy_invite_digest.as_ref().map(|v|v.as_slice())).execute(&mut **tx).await.map_err(database_error)?.rows_affected();
        if legacy != 1 {
            return Err(AccountError::InvalidInvite);
        }
    }
    let user = if let Some(user) = input.requested_uin {
        let occupied: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE uin=?) OR EXISTS(SELECT 1 FROM retired_uins WHERE uin=?)")
            .bind(i64::from(user.get())).bind(i64::from(user.get())).fetch_one(&mut **tx).await.map_err(database_error)?;
        if occupied != 0 {
            return Err(AccountError::NumberTaken);
        }
        user
    } else {
        let mut next: i64 = sqlx::query_scalar("SELECT next_uin FROM number_allocator WHERE id=1")
            .fetch_one(&mut **tx)
            .await
            .map_err(database_error)?;
        loop {
            if next > 0x00ff_ffff {
                return Err(AccountError::NumberRangeExhausted);
            }
            let occupied: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE uin=?) OR EXISTS(SELECT 1 FROM retired_uins WHERE uin=?)")
                .bind(next).bind(next).fetch_one(&mut **tx).await.map_err(database_error)?;
            if occupied == 0 {
                break;
            }
            next += 1;
        }
        sqlx::query("UPDATE number_allocator SET next_uin=? WHERE id=1")
            .bind(next + 1)
            .execute(&mut **tx)
            .await
            .map_err(database_error)?;
        UserId::new(next as u32).map_err(|_| AccountError::Storage)?
    };
    let envelope = protector.seal(user, &input.password)?;
    match sqlx::query("INSERT INTO secret_nonces(key_id,nonce) VALUES(?,?)")
        .bind(&envelope.key_id)
        .bind(envelope.nonce.as_slice())
        .execute(&mut **tx)
        .await
    {
        Ok(_) => (),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            return Err(AccountError::Secret(
                rgg_application::accounts::SecretError::NonceReuse,
            ));
        }
        Err(error) => return Err(database_error(error)),
    }
    sqlx::query("INSERT INTO accounts(uin,email,secret_version,key_id,nonce,ciphertext,created_at) VALUES(?,?,?,?,?,?,unixepoch())")
        .bind(i64::from(user.get())).bind(input.email).bind(i64::from(envelope.version))
        .bind(envelope.key_id).bind(envelope.nonce.as_slice()).bind(envelope.ciphertext)
        .execute(&mut **tx).await.map_err(database_error)?;
    Ok(user)
}

#[async_trait]
impl AccountRepository for SqliteStore {
    async fn create_invite(&self, digest: [u8; 32], uses: u32) -> Result<(), AccountError> {
        if !(1..=1000).contains(&uses) {
            return Err(AccountError::InvalidInvite);
        }
        sqlx::query("INSERT INTO invitations(digest,max_uses,created_at) VALUES(?,?,unixepoch())")
            .bind(digest.as_slice())
            .bind(i64::from(uses))
            .execute(&self.writer)
            .await
            .map_err(database_error)?;
        Ok(())
    }

    async fn register_with_invite(
        &self,
        input: Registration,
        protector: &dyn SecretProtector,
    ) -> Result<UserId, AccountError> {
        input.validate()?;
        let mut tx = self.writer.begin().await.map_err(database_error)?;
        match register(&mut tx, input, protector).await {
            Ok(user) => {
                tx.commit().await.map_err(database_error)?;
                Ok(user)
            }
            Err(error) => {
                tx.rollback().await.map_err(database_error)?;
                Err(error)
            }
        }
    }

    async fn login_credential(&self, user: UserId) -> Result<LoginCredential, AccountError> {
        let row = sqlx::query("SELECT credential_version,secret_version,key_id,nonce,ciphertext FROM accounts WHERE uin=? AND active=1")
            .bind(i64::from(user.get())).fetch_optional(&self.pool).await.map_err(database_error)?.ok_or(AccountError::Inactive)?;
        let nonce: Vec<u8> = row.try_get("nonce").map_err(database_error)?;
        let version: i64 = row.try_get("credential_version").map_err(database_error)?;
        let secret_version: i64 = row.try_get("secret_version").map_err(database_error)?;
        Ok(LoginCredential {
            user,
            credential_version: version.try_into().map_err(|_| AccountError::Storage)?,
            secret: SealedSecret {
                version: secret_version
                    .try_into()
                    .map_err(|_| AccountError::Storage)?,
                key_id: row.try_get("key_id").map_err(database_error)?,
                nonce: nonce.try_into().map_err(|_| AccountError::Storage)?,
                ciphertext: row.try_get("ciphertext").map_err(database_error)?,
            },
        })
    }

    async fn retire_account(&self, user: UserId) -> Result<(), AccountError> {
        let mut tx = self.writer.begin().await.map_err(database_error)?;
        let deleted = sqlx::query("DELETE FROM accounts WHERE uin=? AND NOT EXISTS(SELECT 1 FROM account_sessions WHERE uin=accounts.uin)")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(database_error)?
            .rows_affected();
        if deleted != 1 {
            tx.rollback().await.map_err(database_error)?;
            return Err(AccountError::Inactive);
        }
        sqlx::query("UPDATE deliveries SET state='Rejected',terminal_at=unixepoch(),reason='account_retired' WHERE recipient=? AND state='Pending'").bind(i64::from(user.get())).execute(&mut *tx).await.map_err(database_error)?;
        sqlx::query("UPDATE messages SET body=NULL,styles=NULL WHERE recipient=? AND id IN (SELECT message_id FROM deliveries WHERE state='Rejected')").bind(i64::from(user.get())).execute(&mut *tx).await.map_err(database_error)?;
        sqlx::query("INSERT INTO retired_uins(uin,retired_at) VALUES(?,unixepoch())")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(database_error)?;
        tx.commit().await.map_err(database_error)?;
        Ok(())
    }

    async fn registered_count(&self) -> Result<u64, AccountError> {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
            .fetch_one(&self.pool)
            .await
            .map_err(database_error)?;
        count.try_into().map_err(|_| AccountError::Storage)
    }

    async fn required_key_ids(&self) -> Result<Vec<String>, AccountError> {
        sqlx::query_scalar("SELECT DISTINCT key_id FROM accounts ORDER BY key_id")
            .fetch_all(&self.pool)
            .await
            .map_err(database_error)
    }

    async fn credential_key_probes(&self) -> Result<Vec<LoginCredential>, AccountError> {
        let rows = sqlx::query("SELECT uin,credential_version,secret_version,key_id,nonce,ciphertext FROM accounts WHERE uin IN (SELECT min(uin) FROM accounts GROUP BY key_id)")
            .fetch_all(&self.pool).await.map_err(database_error)?;
        let mut probes = Vec::new();
        for row in rows {
            let number: i64 = row.try_get("uin").map_err(database_error)?;
            let user = UserId::new(number.try_into().map_err(|_| AccountError::Storage)?)
                .map_err(|_| AccountError::Storage)?;
            let version: i64 = row.try_get("credential_version").map_err(database_error)?;
            let secret_version: i64 = row.try_get("secret_version").map_err(database_error)?;
            let nonce: Vec<u8> = row.try_get("nonce").map_err(database_error)?;
            probes.push(LoginCredential {
                user,
                credential_version: version.try_into().map_err(|_| AccountError::Storage)?,
                secret: SealedSecret {
                    version: secret_version
                        .try_into()
                        .map_err(|_| AccountError::Storage)?,
                    key_id: row.try_get("key_id").map_err(database_error)?,
                    nonce: nonce.try_into().map_err(|_| AccountError::Storage)?,
                    ciphertext: row.try_get("ciphertext").map_err(database_error)?,
                },
            });
        }
        Ok(probes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::accounts::{LegacyPassword, SecretError};
    use rgg_credentials::Keyring;
    use std::sync::Arc;
    use zeroize::Zeroizing;

    fn registration(uin: Option<u32>) -> Registration {
        Registration {
            requested_uin: uin.map(|n| UserId::new(n).unwrap()),
            invite_digest: Some([5; 32]),
            legacy_invite_digest: None,
            password: LegacyPassword::new(Zeroizing::new(b"LocalTest9!".to_vec())).unwrap(),
            email: "test@example.invalid".into(),
        }
    }

    async fn setup(uses: u32) -> (tempfile::TempDir, SqliteStore, Keyring) {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(root.path()).await.unwrap();
        let ring = Keyring::create_new(&root.path().join("keys.toml")).unwrap();
        store.create_invite([5; 32], uses).await.unwrap();
        (root, store, ring)
    }

    #[tokio::test]
    async fn manual_numbers_do_not_jump_the_automatic_counter_and_conflicts_do_not_consume_invites()
    {
        let (_root, store, ring) = setup(3).await;
        assert_eq!(
            store
                .register_with_invite(registration(Some(742858)), &ring)
                .await
                .unwrap()
                .get(),
            742858
        );
        assert_eq!(
            store
                .register_with_invite(registration(Some(742858)), &ring)
                .await
                .unwrap_err(),
            AccountError::NumberTaken
        );
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112233
        );
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112234
        );
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap_err(),
            AccountError::InvalidInvite
        );
        assert_eq!(store.registered_count().await.unwrap(), 3);
        let credential = store
            .login_credential(UserId::new(742858).unwrap())
            .await
            .unwrap();
        assert_eq!(
            ring.open(credential.user, &credential.secret)
                .unwrap()
                .expose(),
            b"LocalTest9!"
        );
        assert!(
            !credential
                .secret
                .ciphertext
                .windows(11)
                .any(|bytes| bytes == b"LocalTest9!")
        );
        store.close().await;
    }

    #[tokio::test]
    async fn simultaneous_registrations_obey_the_three_use_limit_and_allocate_unique_numbers() {
        let (_root, store, ring) = setup(3).await;
        let ring = Arc::new(ring);
        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..12 {
            let store = store.clone();
            let ring = ring.clone();
            tasks.spawn(async move {
                store
                    .register_with_invite(registration(None), ring.as_ref())
                    .await
            });
        }
        let mut numbers = Vec::new();
        let mut refused = 0;
        while let Some(result) = tasks.join_next().await {
            match result.unwrap() {
                Ok(user) => numbers.push(user.get()),
                Err(AccountError::InvalidInvite) => refused += 1,
                Err(error) => panic!("{error}"),
            }
        }
        numbers.sort_unstable();
        assert_eq!(numbers, [112233, 112234, 112235]);
        assert_eq!(refused, 9);
        store.close().await;
    }

    #[tokio::test]
    async fn retired_numbers_remain_reserved_after_restart_without_preserving_personal_fields() {
        let (root, store, ring) = setup(3).await;
        let user = store
            .register_with_invite(registration(Some(12)), &ring)
            .await
            .unwrap();
        store.retire_account(user).await.unwrap();
        assert_eq!(store.registered_count().await.unwrap(), 0);
        assert!(matches!(
            store.login_credential(user).await,
            Err(AccountError::Inactive)
        ));
        store.close().await;
        drop(store);
        let store = SqliteStore::open(root.path()).await.unwrap();
        assert_eq!(
            store
                .register_with_invite(registration(Some(12)), &ring)
                .await
                .unwrap_err(),
            AccountError::NumberTaken
        );
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112233
        );
        store.close().await;
    }

    struct BrokenProtector;

    #[tokio::test]
    async fn schema_two_migrates_existing_encrypted_accounts_and_backfills_nonce_history() {
        let root = tempfile::tempdir().unwrap();
        let scripts = root.path().join("old-migrations");
        std::fs::create_dir(&scripts).unwrap();
        std::fs::write(
            scripts.join("0001_runtime.sql"),
            include_str!("../migrations/0001_runtime.sql"),
        )
        .unwrap();
        std::fs::write(
            scripts.join("0002_accounts.sql"),
            include_str!("../migrations/0002_accounts.sql"),
        )
        .unwrap();
        let ring = Keyring::create_new(&root.path().join("keys.toml")).unwrap();
        let user = UserId::new(12).unwrap();
        let secret = ring.seal(user, &registration(None).password).unwrap();
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(root.path().join("rgg.sqlite"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::migrate::Migrator::new(scripts)
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts(uin,email,secret_version,key_id,nonce,ciphertext,created_at) VALUES(12,'test@example.invalid',1,?,?,?,123)")
            .bind(&secret.key_id).bind(secret.nonce.as_slice()).bind(&secret.ciphertext).execute(&pool).await.unwrap();
        pool.close().await;
        let store = SqliteStore::open(root.path()).await.unwrap();
        let loaded = store.login_credential(user).await.unwrap();
        assert_eq!(
            ring.open(user, &loaded.secret).unwrap().expose(),
            b"LocalTest9!"
        );
        let nonces: i64 = sqlx::query_scalar("SELECT count(*) FROM secret_nonces")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(nonces, 1);
        store.integrity_check().await.unwrap();
        store.close().await;
    }

    struct RepeatedEnvelope(SealedSecret);
    impl SecretProtector for RepeatedEnvelope {
        fn seal(&self, _: UserId, _: &LegacyPassword) -> Result<SealedSecret, SecretError> {
            Ok(self.0.clone())
        }
        fn open(&self, _: UserId, _: &SealedSecret) -> Result<LegacyPassword, SecretError> {
            Err(SecretError::Integrity)
        }
    }

    #[tokio::test]
    async fn reused_nonce_is_rejected_even_after_the_old_account_has_been_retired() {
        let (_root, store, ring) = setup(2).await;
        let password = registration(None).password;
        let repeated = RepeatedEnvelope(ring.seal(UserId::new(12).unwrap(), &password).unwrap());
        let first = store
            .register_with_invite(registration(Some(12)), &repeated)
            .await
            .unwrap();
        store.retire_account(first).await.unwrap();
        assert_eq!(
            store
                .register_with_invite(registration(Some(13)), &repeated)
                .await
                .unwrap_err(),
            AccountError::Secret(SecretError::NonceReuse)
        );
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112233
        );
        store.close().await;
    }

    #[tokio::test]
    async fn competing_manual_claims_are_atomic_and_automatic_numbers_skip_occupied_values() {
        let (_root, store, ring) = setup(3).await;
        let (first, second) = tokio::join!(
            store.register_with_invite(registration(Some(112233)), &ring),
            store.register_with_invite(registration(Some(112233)), &ring)
        );
        assert!(matches!(
            (&first, &second),
            (Ok(_), Err(AccountError::NumberTaken)) | (Err(AccountError::NumberTaken), Ok(_))
        ));
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112234
        );
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112235
        );
        store.close().await;
    }
    impl SecretProtector for BrokenProtector {
        fn seal(&self, _: UserId, _: &LegacyPassword) -> Result<SealedSecret, SecretError> {
            Err(SecretError::Random)
        }
        fn open(&self, _: UserId, _: &SealedSecret) -> Result<LegacyPassword, SecretError> {
            Err(SecretError::Integrity)
        }
    }

    #[tokio::test]
    async fn encryption_failure_rolls_back_invitation_and_number_allocation() {
        let (_root, store, ring) = setup(1).await;
        assert!(matches!(
            store
                .register_with_invite(registration(None), &BrokenProtector)
                .await,
            Err(AccountError::Secret(SecretError::Random))
        ));
        assert_eq!(
            store
                .register_with_invite(registration(None), &ring)
                .await
                .unwrap()
                .get(),
            112233
        );
        store.close().await;
    }
}
