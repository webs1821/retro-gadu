// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{
    SqliteStore, database as sqlx,
    messaging::{db, fence, replace_privacy_tx},
};
use async_trait::async_trait;
use rgg_application::{
    messaging::{ContactRule, MessagingError, SessionKey},
    presence::*,
};
use rgg_domain::UserId;
use sqlx::Row;
async fn write_presence(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &SessionKey,
    p: &Presence,
) -> Result<(), MessagingError> {
    let kind = match p.availability {
        Availability::Offline => "offline",
        Availability::Available => "available",
        Availability::Away => "away",
        Availability::Invisible => "invisible",
    };
    sqlx::query("INSERT INTO account_presence(uin,availability,description,return_at,session_id) VALUES(?,?,?,?,?) ON CONFLICT(uin) DO UPDATE SET availability=excluded.availability,description=excluded.description,return_at=excluded.return_at,session_id=excluded.session_id")
        .bind(i64::from(key.user.get())).bind(kind).bind(&p.description).bind(p.return_at.map(i64::from)).bind(if p.availability==Availability::Offline{None}else{Some(key.id.as_slice())}).execute(&mut **tx).await.map_err(db)?;
    sqlx::query("UPDATE account_privacy SET friends_only=?,revision=revision+1 WHERE uin=? AND friends_only!=?")
        .bind(p.friends_only)
        .bind(i64::from(key.user.get()))
        .bind(p.friends_only)
        .execute(&mut **tx)
        .await
        .map_err(db)?;
    Ok(())
}
#[async_trait]
impl PresenceRepository for SqliteStore {
    async fn activate_presence(
        &self,
        key: &SessionKey,
        rules: &[ContactRule],
        presence: &Presence,
        now: i64,
    ) -> Result<(), MessagingError> {
        presence.validate()?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        replace_privacy_tx(&mut tx, key, rules, presence.friends_only, now).await?;
        write_presence(&mut tx, key, presence).await?;
        tx.commit().await.map_err(db)
    }
    async fn set_presence(
        &self,
        key: &SessionKey,
        presence: &Presence,
    ) -> Result<(), MessagingError> {
        presence.validate()?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, key, true).await?;
        write_presence(&mut tx, key, presence).await?;
        tx.commit().await.map_err(db)
    }
    async fn presence_snapshot(
        &self,
        key: &SessionKey,
    ) -> Result<Vec<VisiblePresence>, MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let ready:Option<i64>=sqlx::query_scalar("SELECT s.ready FROM account_sessions s JOIN accounts a ON a.uin=s.uin JOIN runtime_state r ON r.id=1 WHERE s.uin=? AND s.session_id=? AND s.generation=? AND s.credential_version=? AND s.epoch=? AND a.active=1 AND a.session_generation=s.generation AND a.credential_version=s.credential_version AND r.process_epoch=s.epoch")
            .bind(i64::from(key.user.get())).bind(key.id.as_slice()).bind(key.generation).bind(key.credential_version).bind(&key.epoch).fetch_optional(&mut *tx).await.map_err(db)?;
        match ready {
            None => return Err(MessagingError::Stale),
            Some(0) => return Err(MessagingError::NotReady),
            _ => {}
        }
        let rows=sqlx::query("SELECT s.image_limit_kib,watch.peer,watch.flags viewer_flags,COALESCE(policy.flags,0) subject_flags,COALESCE(priv.friends_only,0) friends_only,p.availability,p.description,p.return_at,CASE WHEN p.availability='offline' AND p.session_id IS NULL THEN 1 WHEN s.ready=1 AND s.epoch=? AND s.session_id=p.session_id AND a.active=1 AND a.credential_version=s.credential_version AND a.session_generation=s.generation THEN 1 ELSE 0 END visible FROM contact_rules watch LEFT JOIN accounts a ON a.uin=watch.peer LEFT JOIN account_presence p ON p.uin=watch.peer LEFT JOIN account_sessions s ON s.uin=watch.peer LEFT JOIN account_privacy priv ON priv.uin=watch.peer LEFT JOIN contact_rules policy ON policy.owner=watch.peer AND policy.peer=watch.owner WHERE watch.owner=? AND (watch.flags&1)!=0 ORDER BY watch.peer LIMIT 4001")
            .bind(&key.epoch).bind(i64::from(key.user.get())).fetch_all(&mut *tx).await.map_err(db)?;
        if rows.len() > 4000 {
            return Err(MessagingError::Quota);
        }
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let p = record_presence(&r)?;
            let visible = effective_presence_for(
                &p,
                r.get::<i64, _>("subject_flags") as u8,
                r.get::<i64, _>("viewer_flags") as u8,
            );
            let image_limit_kib = if matches!(
                visible.availability,
                Availability::Available | Availability::Away
            ) {
                r.get::<Option<i64>, _>("image_limit_kib").unwrap_or(0) as u8
            } else {
                0
            };
            out.push(VisiblePresence {
                image_limit_kib,
                user: UserId::new(r.get::<i64, _>("peer") as u32)
                    .map_err(|_| MessagingError::Integrity)?,
                presence: effective_presence_for(
                    &p,
                    r.get::<i64, _>("subject_flags") as u8,
                    r.get::<i64, _>("viewer_flags") as u8,
                ),
            });
        }
        tx.commit().await.map_err(db)?;
        Ok(out)
    }
}

pub(crate) fn record_presence(r: &sqlx::sqlite::SqliteRow) -> Result<Presence, MessagingError> {
    let mut p = Presence::offline();
    if r.get::<i64, _>("visible") == 1 {
        p.availability = match r.get::<Option<String>, _>("availability").as_deref() {
            Some("available") => Availability::Available,
            Some("away") => Availability::Away,
            Some("invisible") => Availability::Invisible,
            _ => Availability::Offline,
        };
        p.description = r.get("description");
        p.return_at = r.get::<Option<i64>, _>("return_at").map(|v| v as u32);
        p.friends_only = r.get::<i64, _>("friends_only") != 0;
        p.validate().map_err(|_| MessagingError::Integrity)?;
    }
    Ok(p)
}
