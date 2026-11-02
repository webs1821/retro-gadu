// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Administrator recovery only while the CLI exclusively owns the data directory.
use crate::{
    SqliteStore, database as sqlx,
    key_maintenance::decode,
    password_maintenance::{PasswordAuditOrigin, write_password_tx},
};
use rgg_application::accounts::{AccountError, LegacyPassword, SecretProtector};
use rgg_domain::UserId;
use sqlx::Row;

fn storage(_: sqlx::Error) -> AccountError {
    AccountError::Storage
}

impl SqliteStore {
    /// Filesystem/keyring access is the authority; the CLI must create a backup first.
    /// Deliberately not part of the online PasswordRepository or any network endpoint.
    pub async fn reset_password_offline(
        &self,
        user: UserId,
        replacement: &LegacyPassword,
        protector: &dyn SecretProtector,
    ) -> Result<i64, AccountError> {
        let mut tx = self.writer.begin().await.map_err(storage)?;
        let owned = sqlx::query(
            "UPDATE runtime_state SET process_epoch=process_epoch WHERE id=1 AND process_epoch=?",
        )
        .bind(&self.epoch)
        .execute(&mut *tx)
        .await
        .map_err(storage)?
        .rows_affected();
        if owned != 1 {
            return Err(AccountError::StaleCredential);
        }
        let live: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_sessions WHERE uin=?)")
                .bind(i64::from(user.get()))
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if live {
            return Err(AccountError::ActiveSession);
        }
        let row = sqlx::query("SELECT uin,credential_version,session_generation,secret_version,key_id,nonce,ciphertext FROM accounts WHERE uin=? AND active=1")
            .bind(i64::from(user.get())).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(AccountError::Inactive)?;
        let previous: i64 = row.try_get("credential_version").map_err(storage)?;
        let generation: i64 = row.try_get("session_generation").map_err(storage)?;
        previous
            .checked_add(1)
            .ok_or(AccountError::StaleCredential)?;
        let next_generation = generation
            .checked_add(1)
            .ok_or(AccountError::StaleCredential)?;
        let (_, old) = decode(&row).map_err(|_| AccountError::Storage)?;
        // Verify the original key and authenticated record even though the operator forgot the password.
        let saved = protector.open(user, &old)?;
        if saved.matches(replacement) {
            return Err(AccountError::UnchangedPassword);
        }
        drop(saved);
        let new = protector.seal(user, replacement)?;
        sqlx::query("UPDATE accounts SET session_generation=? WHERE uin=?")
            .bind(next_generation)
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let now = sqlx::query_scalar("SELECT unixepoch()")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage)?;
        let version = write_password_tx(
            &mut tx,
            user,
            previous,
            &new,
            PasswordAuditOrigin::LocalRecovery,
            now,
        )
        .await?;
        sqlx::query("DELETE FROM password_change_budget WHERE uin=?")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_PASSWORD_RESET_CRASH").as_deref() == Ok("before_reset_commit") {
            std::process::exit(86);
        }
        tx.commit().await.map_err(storage)?;
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_PASSWORD_RESET_CRASH").as_deref() == Ok("after_reset_commit") {
            std::process::exit(86);
        }
        Ok(version)
    }
}
