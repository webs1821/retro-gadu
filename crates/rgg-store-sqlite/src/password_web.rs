// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx, password_maintenance::write_password_tx};
use async_trait::async_trait;
use rgg_application::{
    accounts::AccountError,
    password::{AuthorizedPasswordChange, PasswordRepository},
};
use rgg_domain::UserId;
use sqlx::Row;
fn storage(_: sqlx::Error) -> AccountError {
    AccountError::Storage
}

#[async_trait]
impl PasswordRepository for SqliteStore {
    async fn charge_password_attempt(&self, user: UserId, now: i64) -> Result<bool, AccountError> {
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
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE uin=? AND active=1)")
                .bind(i64::from(user.get()))
                .fetch_one(&mut *tx)
                .await
                .map_err(storage)?;
        if !exists {
            return Ok(false);
        }
        let previous =
            sqlx::query("SELECT window_started,attempts FROM password_change_budget WHERE uin=?")
                .bind(i64::from(user.get()))
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        let (start, attempts) = if let Some(row) = previous {
            let start: i64 = row.try_get("window_started").map_err(storage)?;
            let attempts: i64 = row.try_get("attempts").map_err(storage)?;
            if now.saturating_sub(start) >= 600 {
                (now, 1)
            } else if attempts >= 5 {
                return Ok(false);
            } else {
                (start, attempts + 1)
            }
        } else {
            (now, 1)
        };
        sqlx::query("INSERT INTO password_change_budget(uin,window_started,attempts) VALUES(?,?,?) ON CONFLICT(uin) DO UPDATE SET window_started=excluded.window_started,attempts=excluded.attempts")
            .bind(i64::from(user.get())).bind(start).bind(attempts).execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(true)
    }
    async fn password_change_is_current(
        &self,
        change: &AuthorizedPasswordChange,
    ) -> Result<bool, AccountError> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE uin=? AND active=1 AND credential_version=? AND credential_version<9223372036854775807 AND session_generation<9223372036854775807 AND EXISTS(SELECT 1 FROM runtime_state WHERE id=1 AND process_epoch=?))")
            .bind(i64::from(change.user().get())).bind(change.expected_version()).bind(&self.epoch).fetch_one(&self.pool).await.map_err(storage)
    }
    async fn commit_password_change(
        &self,
        change: &AuthorizedPasswordChange,
        now: i64,
    ) -> Result<i64, AccountError> {
        let mut tx = self.writer.begin().await.map_err(storage)?;
        let changed = sqlx::query("UPDATE accounts SET session_generation=session_generation+1 WHERE uin=? AND active=1 AND credential_version=? AND credential_version<9223372036854775807 AND session_generation<9223372036854775807 AND EXISTS(SELECT 1 FROM runtime_state WHERE id=1 AND process_epoch=?)")
            .bind(i64::from(change.user().get())).bind(change.expected_version()).bind(&self.epoch).execute(&mut *tx).await.map_err(storage)?.rows_affected();
        if changed != 1 {
            return Err(AccountError::StaleCredential);
        }
        let version = write_password_tx(
            &mut tx,
            change.user(),
            change.expected_version(),
            change.secret(),
            "local_web_with_current_password",
            now,
        )
        .await?;
        crate::messaging::release_user(&mut tx, change.user(), now)
            .await
            .map_err(|_| AccountError::Storage)?;
        for statement in [
            "DELETE FROM account_presence WHERE uin=?",
            "DELETE FROM contact_staging WHERE owner=?",
            "DELETE FROM account_sessions WHERE uin=?",
        ] {
            sqlx::query(statement)
                .bind(i64::from(change.user().get()))
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
        }
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_WEB_PASSWORD_CRASH").as_deref()
            == Ok("before_web_password_commit")
        {
            std::process::exit(86);
        }
        tx.commit().await.map_err(storage)?;
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_WEB_PASSWORD_CRASH").as_deref()
            == Ok("after_web_password_commit")
        {
            std::process::exit(86);
        }
        Ok(version)
    }
}
