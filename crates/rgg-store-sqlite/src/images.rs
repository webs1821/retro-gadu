// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{
    SqliteStore, database as sqlx,
    messaging::{db, fence},
};
use async_trait::async_trait;
use rgg_application::{
    images::*,
    messaging::{MessagingError, SessionKey},
};
use rgg_domain::UserId;
use sqlx::Row;
pub(crate) async fn revoke(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    now: i64,
) -> Result<(), MessagingError> {
    sqlx::query("DELETE FROM image_offers WHERE expires_at<=? OR EXISTS(SELECT 1 FROM deliveries d WHERE d.message_id=image_offers.message_id AND d.state IN ('Rejected','RejectedByPrivacy','Expired')) OR EXISTS(SELECT 1 FROM contact_rules r WHERE (r.flags&4)!=0 AND ((r.owner=image_offers.sender AND r.peer=image_offers.recipient) OR (r.owner=image_offers.recipient AND r.peer=image_offers.sender)))")
        .bind(now).execute(&mut **tx).await.map_err(db)?;
    Ok(())
}
#[async_trait]
impl ImageRepository for SqliteStore {
    async fn set_image_limit(&self, key: &SessionKey, kib: u8) -> Result<(), MessagingError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        fence(&mut tx, key, false).await?;
        sqlx::query("UPDATE account_sessions SET image_limit_kib=? WHERE uin=?")
            .bind(i64::from(kib))
            .bind(i64::from(key.user.get()))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)
    }
    async fn authorize_image(
        &self,
        requester: &SessionKey,
        sender: UserId,
        image: ImageId,
        now: i64,
    ) -> Result<ImageRoute, MessagingError> {
        image.validate()?;
        if requester.user == sender {
            return Err(MessagingError::Rejected);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        let row=sqlx::query("SELECT s.uin,s.session_id,s.generation,s.credential_version,s.epoch,t.session_id recipient_id,t.generation recipient_generation,t.credential_version recipient_version,t.epoch recipient_epoch FROM account_sessions s JOIN accounts a ON a.uin=s.uin JOIN account_sessions t ON t.uin=? JOIN accounts b ON b.uin=t.uin JOIN runtime_state r ON r.id=1 WHERE s.uin=? AND s.ready=1 AND t.ready=1 AND a.active=1 AND b.active=1 AND s.epoch=r.process_epoch AND t.epoch=r.process_epoch AND a.session_generation=s.generation AND a.credential_version=s.credential_version AND b.session_generation=t.generation AND b.credential_version=t.credential_version AND t.image_limit_kib*1024>=? AND EXISTS(SELECT 1 FROM image_offers o WHERE o.sender=s.uin AND o.recipient=t.uin AND o.size=? AND o.crc32=? AND o.expires_at>?) AND NOT EXISTS(SELECT 1 FROM contact_rules c WHERE (c.flags&4)!=0 AND ((c.owner=s.uin AND c.peer=t.uin) OR (c.owner=t.uin AND c.peer=s.uin)))")
            .bind(i64::from(requester.user.get())).bind(i64::from(sender.get())).bind(i64::from(image.size)).bind(i64::from(image.size)).bind(i64::from(image.crc32)).bind(now).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(MessagingError::Rejected)?;
        if row.get::<Vec<u8>, _>("recipient_id") != requester.id
            || row.get::<i64, _>("recipient_generation") != requester.generation
            || row.get::<i64, _>("recipient_version") != requester.credential_version
            || row.get::<String, _>("recipient_epoch") != requester.epoch
        {
            return Err(MessagingError::Stale);
        }
        let source = SessionKey {
            user: sender,
            id: row
                .get::<Vec<u8>, _>("session_id")
                .try_into()
                .map_err(|_| MessagingError::Integrity)?,
            generation: row.get("generation"),
            credential_version: row.get("credential_version"),
            epoch: row.get("epoch"),
        };
        tx.commit().await.map_err(db)?;
        Ok(ImageRoute {
            sender: source,
            recipient: requester.clone(),
            image,
        })
    }
    async fn image_route_current(
        &self,
        route: &ImageRoute,
        now: i64,
    ) -> Result<bool, MessagingError> {
        match self
            .authorize_image(&route.recipient, route.sender.user, route.image, now)
            .await
        {
            Ok(current) => Ok(current == *route),
            Err(MessagingError::Rejected | MessagingError::Stale | MessagingError::NotReady) => {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }
}
