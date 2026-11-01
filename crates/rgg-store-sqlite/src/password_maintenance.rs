// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Authenticated offline changes and the shared atomic secret/audit update used by online changes.
use crate::{SqliteStore, database as sqlx, key_maintenance::decode};
use rgg_application::accounts::{
    AccountError, LegacyPassword, SealedSecret, SecretError, SecretProtector,
};
use rgg_domain::UserId;
use sqlx::Row;

fn storage(_: sqlx::Error) -> AccountError {
    AccountError::Storage
}

pub struct PasswordAudit {
    pub id: i64,
    pub actor: String,
    pub user: u32,
    pub previous_version: i64,
    pub credential_version: i64,
    pub occurred_at: i64,
}

pub(crate) async fn write_password_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    user: UserId,
    previous: i64,
    new: &SealedSecret,
    actor: &'static str,
    now: i64,
) -> Result<i64, AccountError> {
    let version = previous
        .checked_add(1)
        .ok_or(AccountError::StaleCredential)?;
    match sqlx::query("INSERT INTO secret_nonces(key_id,nonce) VALUES(?,?)")
        .bind(&new.key_id)
        .bind(new.nonce.as_slice())
        .execute(&mut **tx)
        .await
    {
        Ok(_) => (),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(SecretError::NonceReuse.into());
        }
        Err(e) => return Err(storage(e)),
    }
    let changed = sqlx::query("UPDATE accounts SET credential_version=?,secret_version=?,key_id=?,nonce=?,ciphertext=? WHERE uin=? AND credential_version=? AND active=1")
        .bind(version).bind(i64::from(new.version)).bind(&new.key_id).bind(new.nonce.as_slice()).bind(new.ciphertext.as_slice())
        .bind(i64::from(user.get())).bind(previous).execute(&mut **tx).await.map_err(storage)?.rows_affected();
    if changed != 1 {
        return Err(AccountError::StaleCredential);
    }
    sqlx::query("INSERT INTO admin_audit(actor,action,target_uin,previous_version,credential_version,occurred_at) VALUES(?,'password_changed',?,?,?,?)")
        .bind(actor).bind(i64::from(user.get())).bind(previous).bind(version).bind(now).execute(&mut **tx).await.map_err(storage)?;
    sqlx::query("DELETE FROM admin_audit WHERE occurred_at<?")
        .bind(now.saturating_sub(2592000))
        .execute(&mut **tx)
        .await
        .map_err(storage)?;
    sqlx::query("DELETE FROM admin_audit WHERE id IN (SELECT id FROM admin_audit ORDER BY id DESC LIMIT -1 OFFSET 10000)").execute(&mut **tx).await.map_err(storage)?;
    Ok(version)
}

impl SqliteStore {
    /// Only for the offline CLI while it exclusively owns the directory. This is not a reset.
    pub async fn change_password_offline(
        &self,
        user: UserId,
        current: &LegacyPassword,
        replacement: &LegacyPassword,
        protector: &dyn SecretProtector,
    ) -> Result<i64, AccountError> {
        let mut tx = self.writer.begin().await.map_err(storage)?;
        sqlx::query("UPDATE runtime_state SET process_epoch=process_epoch WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let live: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_sessions WHERE uin=?)")
                .bind(i64::from(user.get()))
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if live {
            return Err(AccountError::ActiveSession);
        }
        let row=sqlx::query("SELECT uin,credential_version,secret_version,key_id,nonce,ciphertext FROM accounts WHERE uin=? AND active=1")
            .bind(i64::from(user.get())).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(AccountError::Inactive)?;
        let (_, old) = decode(&row).map_err(|_| AccountError::Storage)?;
        let saved = protector.open(user, &old)?;
        if !saved.matches(current) {
            return Err(AccountError::IncorrectPassword);
        }
        if saved.matches(replacement) {
            return Err(AccountError::UnchangedPassword);
        }
        drop(saved);
        let previous: i64 = row.try_get("credential_version").map_err(storage)?;
        let new = protector.seal(user, replacement)?;
        let now = sqlx::query_scalar("SELECT unixepoch()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let version = write_password_tx(
            &mut tx,
            user,
            previous,
            &new,
            "local_cli_with_current_password",
            now,
        )
        .await?;
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_PASSWORD_CRASH").as_deref() == Ok("before_password_commit") {
            std::process::exit(86);
        }
        tx.commit().await.map_err(storage)?;
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_PASSWORD_CRASH").as_deref() == Ok("after_password_commit") {
            std::process::exit(86);
        }
        Ok(version)
    }
    pub async fn password_audit(&self) -> Result<Vec<PasswordAudit>, AccountError> {
        let rows=sqlx::query("SELECT id,actor,target_uin,previous_version,credential_version,occurred_at FROM admin_audit WHERE occurred_at>=unixepoch()-2592000 ORDER BY id DESC LIMIT 100")
            .fetch_all(&self.pool).await.map_err(storage)?;
        rows.into_iter()
            .map(|r| {
                Ok(PasswordAudit {
                    id: r.try_get("id").map_err(storage)?,
                    actor: r.try_get("actor").map_err(storage)?,
                    user: r
                        .try_get::<i64, _>("target_uin")
                        .map_err(storage)?
                        .try_into()
                        .map_err(|_| AccountError::Storage)?,
                    previous_version: r.try_get("previous_version").map_err(storage)?,
                    credential_version: r.try_get("credential_version").map_err(storage)?,
                    occurred_at: r.try_get("occurred_at").map_err(storage)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rgg_application::{
        accounts::{AccountRepository, Registration, SealedSecret},
        messaging::{MessagingError, MessagingRepository},
    };
    use rgg_credentials::Keyring;
    use zeroize::Zeroizing;
    fn password(s: &str) -> LegacyPassword {
        LegacyPassword::new(Zeroizing::new(s.as_bytes().to_vec())).unwrap()
    }
    async fn setup() -> (tempfile::TempDir, SqliteStore, Keyring, UserId) {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(root.path()).await.unwrap();
        let ring = Keyring::create_new(&root.path().join("keys.toml")).unwrap();
        store.create_invite([1; 32], 1).await.unwrap();
        let u = store
            .register_with_invite(
                Registration {
                    requested_uin: Some(UserId::new(1).unwrap()),
                    invite_digest: Some([1; 32]),
                    legacy_invite_digest: None,
                    password: password("LocalTest9!"),
                    email: "synthetic@example.invalid".into(),
                },
                &ring,
            )
            .await
            .unwrap();
        (root, store, ring, u)
    }
    struct Reuse<'a>(&'a Keyring, SealedSecret);
    impl SecretProtector for Reuse<'_> {
        fn open(&self, u: UserId, s: &SealedSecret) -> Result<LegacyPassword, SecretError> {
            self.0.open(u, s)
        }
        fn seal(&self, _: UserId, _: &LegacyPassword) -> Result<SealedSecret, SecretError> {
            Ok(self.1.clone())
        }
    }
    #[tokio::test]
    async fn authenticates_full_password_and_rolls_back_nonce_failure() {
        let (_root, store, ring, u) = setup().await;
        let old = password("LocalTest9!");
        let new = password("NewLocalTest8!");
        let original = store.login_credential(u).await.unwrap();
        assert_eq!(
            store
                .change_password_offline(u, &password("WrongTest9!"), &new, &ring)
                .await
                .unwrap_err(),
            AccountError::IncorrectPassword
        );
        assert_eq!(
            store
                .change_password_offline(u, &old, &old, &ring)
                .await
                .unwrap_err(),
            AccountError::UnchangedPassword
        );
        assert_eq!(
            store
                .change_password_offline(u, &old, &new, &Reuse(&ring, original.secret.clone()))
                .await
                .unwrap_err(),
            AccountError::Secret(SecretError::NonceReuse)
        );
        assert_eq!(
            store.login_credential(u).await.unwrap().credential_version,
            1
        );
        assert!(store.password_audit().await.unwrap().is_empty());
        assert_eq!(
            store
                .change_password_offline(u, &old, &new, &ring)
                .await
                .unwrap(),
            2
        );
        let changed = store.login_credential(u).await.unwrap();
        assert!(ring.open(u, &changed.secret).unwrap().matches(&new));
        assert_ne!(original.secret.nonce, changed.secret.nonce);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM secret_nonces")
                .fetch_one(&store.pool)
                .await
                .unwrap(),
            2
        );
        assert_eq!(
            store.password_audit().await.unwrap()[0].credential_version,
            2
        );
        assert_eq!(
            store
                .attach_session_if_current(u, 1, [1; 16], 1)
                .await
                .unwrap_err(),
            MessagingError::Stale
        );
        store.close().await;
    }
    #[tokio::test]
    async fn refuses_live_inactive_accounts_and_version_overflow() {
        let (_root, store, ring, u) = setup().await;
        let old = password("LocalTest9!");
        let new = password("NewLocalTest8!");
        let session = store
            .attach_session_if_current(u, 1, [1; 16], 1)
            .await
            .unwrap();
        assert_eq!(
            store
                .change_password_offline(u, &old, &new, &ring)
                .await
                .unwrap_err(),
            AccountError::ActiveSession
        );
        store.detach_session(&session, 1).await.unwrap();
        sqlx::query("UPDATE accounts SET active=0")
            .execute(&store.writer)
            .await
            .unwrap();
        assert_eq!(
            store
                .change_password_offline(u, &old, &new, &ring)
                .await
                .unwrap_err(),
            AccountError::Inactive
        );
        sqlx::query("UPDATE accounts SET active=1,credential_version=9223372036854775807")
            .execute(&store.writer)
            .await
            .unwrap();
        assert_eq!(
            store
                .change_password_offline(u, &old, &new, &ring)
                .await
                .unwrap_err(),
            AccountError::StaleCredential
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM secret_nonces")
                .fetch_one(&store.pool)
                .await
                .unwrap(),
            1
        );
        store.close().await;
    }
    #[tokio::test]
    async fn bounds_audit_count_and_age_without_exposing_expired_events() {
        let (_root, store, ring, u) = setup().await;
        sqlx::query("INSERT INTO admin_audit(actor,action,target_uin,previous_version,credential_version,occurred_at) VALUES('local_cli_with_current_password','password_changed',1,1,2,0)").execute(&store.writer).await.unwrap();
        assert!(store.password_audit().await.unwrap().is_empty());
        sqlx::query("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10001) INSERT INTO admin_audit(actor,action,target_uin,previous_version,credential_version,occurred_at) SELECT 'local_cli_with_current_password','password_changed',1,1,2,unixepoch() FROM n").execute(&store.writer).await.unwrap();
        store
            .change_password_offline(
                u,
                &password("LocalTest9!"),
                &password("NewLocalTest8!"),
                &ring,
            )
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_audit")
                .fetch_one(&store.pool)
                .await
                .unwrap(),
            10000
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admin_audit WHERE occurred_at=0")
                .fetch_one(&store.pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(store.password_audit().await.unwrap().len(), 100);
        store.close().await;
    }
}
