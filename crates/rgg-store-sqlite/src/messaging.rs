// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx};
use async_trait::async_trait;
use rand::TryRngCore;
use rgg_application::messaging::*;
use rgg_domain::{MessageText, UserId};
use sqlx::{Row, Sqlite, Transaction};

pub(crate) fn db(_: sqlx::Error) -> MessagingError {
    MessagingError::Storage
}
type Tx<'a> = Transaction<'a, Sqlite>;

pub(crate) async fn fence(
    tx: &mut Tx<'_>,
    key: &SessionKey,
    require_ready: bool,
) -> Result<(), MessagingError> {
    let ready: Option<i64> = sqlx::query_scalar("UPDATE account_sessions SET ready=ready WHERE uin=? AND session_id=? AND generation=? AND credential_version=? AND epoch=? AND EXISTS(SELECT 1 FROM accounts a WHERE a.uin=account_sessions.uin AND a.active=1 AND a.credential_version=account_sessions.credential_version AND a.session_generation=account_sessions.generation) AND EXISTS(SELECT 1 FROM runtime_state WHERE id=1 AND process_epoch=account_sessions.epoch) RETURNING ready")
        .bind(i64::from(key.user.get())).bind(key.id.as_slice()).bind(key.generation).bind(key.credential_version).bind(&key.epoch)
        .fetch_optional(&mut **tx).await.map_err(db)?;
    match ready {
        None => Err(MessagingError::Stale),
        Some(0) if require_ready => Err(MessagingError::NotReady),
        _ => Ok(()),
    }
}

async fn purge_bodies(tx: &mut Tx<'_>) -> Result<(), MessagingError> {
    sqlx::query("UPDATE messages SET body=NULL,styles=NULL WHERE body IS NOT NULL AND id IN (SELECT message_id FROM deliveries WHERE state NOT IN ('Pending','Leased'))")
        .execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
async fn privacy_rejections(tx: &mut Tx<'_>, user: UserId, now: i64) -> Result<(), MessagingError> {
    sqlx::query("UPDATE deliveries SET state='RejectedByPrivacy',terminal_at=?,reason='blocked' WHERE recipient=? AND state='Pending' AND message_id IN (SELECT m.id FROM messages m JOIN contact_rules r ON r.owner=m.recipient AND r.peer=m.sender WHERE (r.flags & 4)!=0)")
        .bind(now).bind(i64::from(user.get())).execute(&mut **tx).await.map_err(db)?;
    purge_bodies(tx).await
}
async fn expire(tx: &mut Tx<'_>, now: i64) -> Result<(), MessagingError> {
    // A live writer owns Leased until its transport barrier, even if UTC jumps forward.
    sqlx::query("UPDATE deliveries SET state='Expired',terminal_at=?,reason='ttl' WHERE state='Pending' AND message_id IN (SELECT id FROM messages WHERE expires_at<=?)")
        .bind(now).bind(now).execute(&mut **tx).await.map_err(db)?;
    purge_bodies(tx).await?;
    sqlx::query("DELETE FROM messages WHERE id IN (SELECT message_id FROM deliveries WHERE terminal_at IS NOT NULL AND terminal_at<?)")
        .bind(now.saturating_sub(7*86400)).execute(&mut **tx).await.map_err(db)?;
    // Retention has a cardinality cap as well as a time limit; repeated successful deliveries
    // must not bypass the active-queue budget by growing diagnostic history indefinitely.
    sqlx::query("DELETE FROM messages WHERE id IN (SELECT message_id FROM deliveries WHERE terminal_at IS NOT NULL ORDER BY terminal_at DESC,message_id DESC LIMIT -1 OFFSET 100000)").execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
async fn release_user(tx: &mut Tx<'_>, user: UserId, now: i64) -> Result<(), MessagingError> {
    sqlx::query("UPDATE delivery_attempts SET outcome='Interrupted',completed_at=? WHERE outcome IS NULL AND message_id IN (SELECT message_id FROM deliveries WHERE recipient=? AND state='Leased')")
        .bind(now).bind(i64::from(user.get())).execute(&mut **tx).await.map_err(db)?;
    sqlx::query("UPDATE deliveries SET state='Pending',lease_token=NULL,session_id=NULL,epoch=NULL,generation=NULL,lease_until=NULL WHERE recipient=? AND state='Leased'")
        .bind(i64::from(user.get())).execute(&mut **tx).await.map_err(db)?;
    Ok(())
}

pub(crate) async fn replace_privacy_tx(
    tx: &mut Tx<'_>,
    session: &SessionKey,
    rules: &[ContactRule],
    friends_only: bool,
    now: i64,
) -> Result<(), MessagingError> {
    if rules.len() > 4000 || rules.iter().any(|r| r.flags & !7 != 0) {
        return Err(MessagingError::Invalid);
    }
    fence(tx, session, false).await?;
    sqlx::query("DELETE FROM contact_rules WHERE owner=?")
        .bind(i64::from(session.user.get()))
        .execute(&mut **tx)
        .await
        .map_err(db)?;
    for rule in rules {
        sqlx::query("INSERT INTO contact_rules(owner,peer,flags) VALUES(?,?,?) ON CONFLICT(owner,peer) DO UPDATE SET flags=(flags | excluded.flags)")
                .bind(i64::from(session.user.get())).bind(i64::from(rule.user.get())).bind(i64::from(rule.flags)).execute(&mut **tx).await.map_err(db)?;
    }
    sqlx::query("UPDATE account_privacy SET friends_only=?,revision=revision+1 WHERE uin=?")
        .bind(friends_only)
        .bind(i64::from(session.user.get()))
        .execute(&mut **tx)
        .await
        .map_err(db)?;
    privacy_rejections(tx, session.user, now).await?;
    sqlx::query("UPDATE account_sessions SET ready=1 WHERE uin=?")
        .bind(i64::from(session.user.get()))
        .execute(&mut **tx)
        .await
        .map_err(db)?;
    Ok(())
}

#[async_trait]
impl MessagingRepository for SqliteStore {
    async fn attach_session_if_current(
        &self,
        user: UserId,
        credential_version: i64,
        id: [u8; 16],
        now: i64,
    ) -> Result<SessionKey, MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let generation: Option<i64> = sqlx::query_scalar("UPDATE accounts SET session_generation=session_generation+1 WHERE uin=? AND active=1 AND credential_version=? AND session_generation<9223372036854775807 RETURNING session_generation")
            .bind(i64::from(user.get())).bind(credential_version).fetch_optional(&mut *tx).await.map_err(db)?;
        let generation = generation.ok_or(MessagingError::Stale)?;
        // Application must have crossed the previous transport's stopped barrier first.
        release_user(&mut tx, user, now).await?;
        sqlx::query("DELETE FROM account_presence WHERE uin=?")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM contact_staging WHERE owner=?")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("INSERT OR IGNORE INTO account_privacy(uin) VALUES(?)")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("INSERT INTO account_sessions(uin,session_id,epoch,generation,credential_version,ready) VALUES(?,?,?,?,?,0) ON CONFLICT(uin) DO UPDATE SET session_id=excluded.session_id,epoch=excluded.epoch,generation=excluded.generation,credential_version=excluded.credential_version,ready=0")
            .bind(i64::from(user.get())).bind(id.as_slice()).bind(&self.epoch).bind(generation).bind(credential_version).execute(&mut *tx).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(SessionKey {
            user,
            id,
            generation,
            credential_version,
            epoch: self.epoch.clone(),
        })
    }
    async fn session_is_current(&self, session: &SessionKey) -> Result<bool, MessagingError> {
        let found: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_sessions s JOIN accounts a ON a.uin=s.uin JOIN runtime_state r ON r.id=1 WHERE s.uin=? AND s.session_id=? AND s.generation=? AND s.credential_version=? AND s.epoch=? AND a.active=1 AND a.credential_version=s.credential_version AND a.session_generation=s.generation AND r.process_epoch=s.epoch)")
            .bind(i64::from(session.user.get())).bind(session.id.as_slice()).bind(session.generation).bind(session.credential_version).bind(&session.epoch).fetch_one(&self.pool).await.map_err(db)?;
        Ok(found == 1)
    }
    async fn detach_session(&self, session: &SessionKey, now: i64) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, session, false).await?;
        release_user(&mut tx, session.user, now).await?;
        sqlx::query("DELETE FROM account_presence WHERE uin=? AND availability!='offline'")
            .bind(i64::from(session.user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM contact_staging WHERE owner=? AND session_id=?")
            .bind(i64::from(session.user.get()))
            .bind(session.id.as_slice())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM account_sessions WHERE uin=? AND session_id=?")
            .bind(i64::from(session.user.get()))
            .bind(session.id.as_slice())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }
    async fn begin_initial_state(&self, session: &SessionKey) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, session, false).await?;
        sqlx::query("UPDATE account_sessions SET ready=0 WHERE uin=?")
            .bind(i64::from(session.user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }
    async fn replace_privacy(
        &self,
        session: &SessionKey,
        rules: &[ContactRule],
        friends_only: bool,
        now: i64,
    ) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        replace_privacy_tx(&mut tx, session, rules, friends_only, now).await?;
        tx.commit().await.map_err(db)
    }
    async fn change_contact(
        &self,
        session: &SessionKey,
        rule: ContactRule,
        remove: bool,
        now: i64,
    ) -> Result<(), MessagingError> {
        if rule.flags & !7 != 0 {
            return Err(MessagingError::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, session, true).await?;
        if remove {
            sqlx::query("UPDATE contact_rules SET flags=(flags & ?) WHERE owner=? AND peer=?")
                .bind(i64::from(!rule.flags & 7))
                .bind(i64::from(session.user.get()))
                .bind(i64::from(rule.user.get()))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        } else {
            let count: i64 =
                sqlx::query_scalar("SELECT count(*) FROM contact_rules WHERE owner=? AND peer!=?")
                    .bind(i64::from(session.user.get()))
                    .bind(i64::from(rule.user.get()))
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db)?;
            if count >= 4000 {
                return Err(MessagingError::Quota);
            }
            sqlx::query("INSERT INTO contact_rules(owner,peer,flags) VALUES(?,?,?) ON CONFLICT(owner,peer) DO UPDATE SET flags=(flags | excluded.flags)")
                .bind(i64::from(session.user.get())).bind(i64::from(rule.user.get())).bind(i64::from(rule.flags)).execute(&mut *tx).await.map_err(db)?;
        }
        sqlx::query("DELETE FROM contact_rules WHERE owner=? AND flags=0")
            .bind(i64::from(session.user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("UPDATE account_privacy SET revision=revision+1 WHERE uin=?")
            .bind(i64::from(session.user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        privacy_rejections(&mut tx, session.user, now).await?;
        tx.commit().await.map_err(db)
    }
    async fn change_friends_only(
        &self,
        session: &SessionKey,
        value: bool,
    ) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, session, false).await?;
        // Friends-only governs presence visibility, not inbound message blocking.
        sqlx::query("UPDATE account_privacy SET friends_only=?,revision=revision+1 WHERE uin=?")
            .bind(value)
            .bind(i64::from(session.user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }
    async fn accept_message_bundle(
        &self,
        session: &SessionKey,
        message: MessageBundle,
        now: i64,
        limits: &QueueLimits,
    ) -> Result<i64, MessagingError> {
        if message.presentation & !12 != 0
            || message.styles.len() > 256
            || message
                .styles
                .iter()
                .any(|s| usize::from(s.position) > message.text.as_str().chars().count())
            || !(0..=i64::from(u32::MAX)).contains(&now)
        {
            return Err(MessagingError::Invalid);
        }
        let styles = serde_json::to_string(&message.styles).map_err(|_| MessagingError::Invalid)?;
        let size = message.text.as_str().len() + styles.len();
        if size > 48 * 1024 {
            return Err(MessagingError::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, session, true).await?;
        let permitted:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts WHERE uin=? AND active=1) AND NOT EXISTS(SELECT 1 FROM contact_rules WHERE owner=? AND peer=? AND (flags&4)!=0)")
            .bind(i64::from(message.recipient.get())).bind(i64::from(message.recipient.get())).bind(i64::from(session.user.get())).fetch_one(&mut *tx).await.map_err(db)?;
        if permitted == 0 {
            return Err(MessagingError::Rejected);
        }
        expire(&mut tx, now).await?;
        let row=sqlx::query("SELECT count(*) AS total,COALESCE(sum(m.payload_bytes),0) AS bytes,COALESCE(sum(m.recipient=?),0) AS recipient_count,COALESCE(sum(CASE WHEN m.recipient=? THEN m.payload_bytes ELSE 0 END),0) AS recipient_bytes,COALESCE(sum(m.sender=?),0) AS sender_count FROM messages m JOIN deliveries d ON d.message_id=m.id WHERE d.state IN ('Pending','Leased')")
            .bind(i64::from(message.recipient.get())).bind(i64::from(message.recipient.get())).bind(i64::from(session.user.get())).fetch_one(&mut *tx).await.map_err(db)?;
        if row.get::<i64, _>("total") >= i64::from(limits.global_messages)
            || row.get::<i64, _>("bytes") + size as i64 > i64::from(limits.global_bytes)
            || row.get::<i64, _>("recipient_count") >= i64::from(limits.per_recipient_messages)
            || row.get::<i64, _>("recipient_bytes") + size as i64
                > i64::from(limits.per_recipient_bytes)
            || row.get::<i64, _>("sender_count") >= i64::from(limits.per_sender_messages)
        {
            return Err(MessagingError::Quota);
        }
        let target: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT session_id FROM account_sessions WHERE uin=? AND ready=1 AND epoch=?",
        )
        .bind(i64::from(message.recipient.get()))
        .bind(&self.epoch)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let id=sqlx::query("INSERT INTO messages(sender,recipient,client_sequence,payload_version,presentation,body,styles,payload_bytes,accepted_at,expires_at,target_session) VALUES(?,?,?,1,?,?,?,?,?,?,?)")
            .bind(i64::from(session.user.get())).bind(i64::from(message.recipient.get())).bind(i64::from(message.client_sequence)).bind(i64::from(message.presentation))
            .bind(message.text.as_str()).bind(styles).bind(size as i64).bind(now).bind(now+i64::from(limits.ttl_seconds)).bind(target).execute(&mut *tx).await.map_err(db)?.last_insert_rowid();
        sqlx::query("INSERT INTO deliveries(message_id,recipient,enqueue_order,state) VALUES(?,?,?,'Pending')").bind(id).bind(i64::from(message.recipient.get())).bind(id).execute(&mut *tx).await.map_err(db)?;
        #[cfg(feature = "test-hooks")]
        if message.client_sequence == 0xfeed
            && std::env::var("RGG_TEST_CRASH").as_deref() == Ok("before_accept_commit")
        {
            std::process::exit(86);
        }
        tx.commit().await.map_err(db)?;
        Ok(id)
    }
    async fn claim_delivery(
        &self,
        session: &SessionKey,
        now: i64,
    ) -> Result<Option<DeliveryLease>, MessagingError> {
        if !self.session_is_current(session).await? {
            return Err(MessagingError::Stale);
        }
        let waiting:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM deliveries WHERE recipient=? AND state IN ('Pending','Leased'))").bind(i64::from(session.user.get())).fetch_one(&self.pool).await.map_err(db)?;
        if waiting == 0 {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, session, true).await?;
        expire(&mut tx, now).await?;
        privacy_rejections(&mut tx, session.user, now).await?;
        let row=sqlx::query("SELECT m.*,d.state FROM deliveries d JOIN messages m ON m.id=d.message_id WHERE d.recipient=? AND d.state IN ('Pending','Leased') ORDER BY d.enqueue_order LIMIT 1")
            .bind(i64::from(session.user.get())).fetch_optional(&mut *tx).await.map_err(db)?;
        let Some(row) = row else {
            tx.commit().await.map_err(db)?;
            return Ok(None);
        };
        if row.get::<String, _>("state") == "Leased" {
            return Ok(None);
        }
        let id: i64 = row.get("id");
        let text: Option<String> = row.get("body");
        let encoded: Option<String> = row.get("styles");
        let parsed = text
            .and_then(|v| MessageText::new(v).ok())
            .zip(encoded.and_then(|v| serde_json::from_str::<Vec<TextStyle>>(&v).ok()));
        let Some((text, styles)) = parsed else {
            sqlx::query("UPDATE deliveries SET state='Rejected',terminal_at=?,reason='payload_integrity' WHERE message_id=? AND state='Pending'").bind(now).bind(id).execute(&mut *tx).await.map_err(db)?;
            purge_bodies(&mut tx).await?;
            tx.commit().await.map_err(db)?;
            return Err(MessagingError::Integrity);
        };
        let mut token = [0; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut token)
            .map_err(|_| MessagingError::Storage)?;
        sqlx::query("UPDATE deliveries SET state='Leased',lease_token=?,session_id=?,epoch=?,generation=?,lease_until=? WHERE message_id=? AND state='Pending'")
            .bind(token.as_slice()).bind(session.id.as_slice()).bind(&session.epoch).bind(session.generation).bind(now+30).bind(id).execute(&mut *tx).await.map_err(db)?;
        sqlx::query("INSERT INTO delivery_attempts(message_id,token,session_id,epoch,generation,started_at) VALUES(?,?,?,?,?,?)")
            .bind(id).bind(token.as_slice()).bind(session.id.as_slice()).bind(&session.epoch).bind(session.generation).bind(now).execute(&mut *tx).await.map_err(db)?;
        sqlx::query("DELETE FROM delivery_attempts WHERE message_id=? AND outcome IS NOT NULL AND id NOT IN (SELECT id FROM delivery_attempts WHERE message_id=? ORDER BY id DESC LIMIT 32)").bind(id).bind(id).execute(&mut *tx).await.map_err(db)?;
        let target: Option<Vec<u8>> = row.get("target_session");
        let lease = DeliveryLease {
            id,
            token,
            sender: UserId::new(row.get::<i64, _>("sender") as u32)
                .map_err(|_| MessagingError::Integrity)?,
            client_sequence: row.get::<i64, _>("client_sequence") as u32,
            accepted_at: row.get("accepted_at"),
            presentation: row.get::<i64, _>("presentation") as u8,
            queued: target.as_deref() != Some(session.id.as_slice()),
            text,
            styles,
        };
        tx.commit().await.map_err(db)?;
        Ok(Some(lease))
    }
    async fn record_transport_write(
        &self,
        session: &SessionKey,
        lease: &DeliveryLease,
        now: i64,
    ) -> Result<(), MessagingError> {
        transition(self, session, lease, now, true).await
    }
    async fn release_delivery(
        &self,
        session: &SessionKey,
        lease: &DeliveryLease,
        now: i64,
    ) -> Result<(), MessagingError> {
        transition(self, session, lease, now, false).await
    }
    async fn prune(&self, now: i64) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        expire(&mut tx, now).await?;
        tx.commit().await.map_err(db)
    }
    async fn revoke_credentials(
        &self,
        user: UserId,
        version: i64,
        now: i64,
    ) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let count=sqlx::query("UPDATE accounts SET credential_version=credential_version+1,session_generation=session_generation+1 WHERE uin=? AND credential_version=? AND credential_version<9223372036854775807 AND session_generation<9223372036854775807")
            .bind(i64::from(user.get())).bind(version).execute(&mut *tx).await.map_err(db)?.rows_affected();
        if count != 1 {
            return Err(MessagingError::Stale);
        }
        release_user(&mut tx, user, now).await?;
        sqlx::query("DELETE FROM account_presence WHERE uin=?")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM contact_staging WHERE owner=?")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM account_sessions WHERE uin=?")
            .bind(i64::from(user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }
}

async fn transition(
    store: &SqliteStore,
    session: &SessionKey,
    lease: &DeliveryLease,
    now: i64,
    written: bool,
) -> Result<(), MessagingError> {
    let mut tx = store.pool.begin().await.map_err(db)?;
    fence(&mut tx, session, true).await?;
    let state = if written {
        "TransportWrittenUnconfirmed"
    } else {
        "Pending"
    };
    let changed=sqlx::query("UPDATE deliveries SET state=?,terminal_at=?,lease_token=NULL,session_id=NULL,epoch=NULL,generation=NULL,lease_until=NULL WHERE message_id=? AND recipient=? AND state='Leased' AND lease_token=? AND session_id=? AND epoch=? AND generation=?")
        .bind(state).bind(if written {Some(now)}else{None}).bind(lease.id).bind(i64::from(session.user.get())).bind(lease.token.as_slice()).bind(session.id.as_slice()).bind(&session.epoch).bind(session.generation)
        .execute(&mut *tx).await.map_err(db)?.rows_affected();
    if changed != 1 {
        return Err(MessagingError::Stale);
    }
    sqlx::query(
        "UPDATE delivery_attempts SET outcome=?,completed_at=? WHERE token=? AND outcome IS NULL",
    )
    .bind(if written {
        "TransportWrittenUnconfirmed"
    } else {
        "Interrupted"
    })
    .bind(now)
    .bind(lease.token.as_slice())
    .execute(&mut *tx)
    .await
    .map_err(db)?;
    if written {
        purge_bodies(&mut tx).await?;
    }
    tx.commit().await.map_err(db)
}
