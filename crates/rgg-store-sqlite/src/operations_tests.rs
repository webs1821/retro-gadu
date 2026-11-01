// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{database as sqlx, messaging_tests::setup};
use rgg_application::{
    accounts::AccountRepository,
    messaging::*,
    operations::OperationalRepository,
    presence::{Availability, Presence, PresenceRepository},
    statistics::StatisticsRepository,
};
use rgg_domain::{MessageText, UserId};

#[tokio::test]
async fn operator_sessions_include_private_users_but_never_stale_sessions() {
    let (_root, store, _) = setup().await;
    let user = UserId::new(1).unwrap();
    let key = store
        .attach_session_if_current(user, 1, [1; 16], 100)
        .await
        .unwrap();
    let sample = store.operational_snapshot().await.unwrap();
    assert_eq!(
        (
            sample.active_accounts,
            sample.ready_sessions,
            sample.synchronizing_sessions
        ),
        (3, 0, 1)
    );
    let presence = Presence {
        availability: Availability::Invisible,
        description: Some("PrivateCanary".into()),
        return_at: None,
        friends_only: true,
    };
    store
        .activate_presence(&key, &[], &presence, 100)
        .await
        .unwrap();
    assert_eq!(store.public_statistics().await.unwrap().publicly_online, 0);
    assert_eq!(
        store.operational_snapshot().await.unwrap().ready_sessions,
        1
    );
    for (break_fence, restore) in [
        (
            "UPDATE accounts SET active=0 WHERE uin=1",
            "UPDATE accounts SET active=1 WHERE uin=1",
        ),
        (
            "UPDATE accounts SET credential_version=2 WHERE uin=1",
            "UPDATE accounts SET credential_version=1 WHERE uin=1",
        ),
        (
            "UPDATE accounts SET session_generation=session_generation+1 WHERE uin=1",
            "UPDATE accounts SET session_generation=session_generation-1 WHERE uin=1",
        ),
        (
            "UPDATE account_sessions SET epoch='old' WHERE uin=1",
            "UPDATE account_sessions SET epoch=(SELECT process_epoch FROM runtime_state WHERE id=1) WHERE uin=1",
        ),
        (
            "UPDATE account_presence SET session_id=zeroblob(16) WHERE uin=1",
            "UPDATE account_presence SET session_id=(SELECT session_id FROM account_sessions WHERE uin=1) WHERE uin=1",
        ),
    ] {
        sqlx::query(break_fence).execute(&store.pool).await.unwrap();
        let sample = store.operational_snapshot().await.unwrap();
        assert_eq!(
            (sample.ready_sessions, sample.synchronizing_sessions),
            (0, 0)
        );
        sqlx::query(restore).execute(&store.pool).await.unwrap();
    }
    store.detach_session(&key, 101).await.unwrap();
    store.retire_account(user).await.unwrap();
    sqlx::query("UPDATE accounts SET active=0 WHERE uin=2")
        .execute(&store.pool)
        .await
        .unwrap();
    let sample = store.operational_snapshot().await.unwrap();
    assert_eq!(
        (
            sample.active_accounts,
            sample.disabled_accounts,
            sample.retired_numbers
        ),
        (1, 1, 1)
    );
    for n in [2, 3] {
        store.retire_account(UserId::new(n).unwrap()).await.unwrap();
    }
    let empty = store.operational_snapshot().await.unwrap();
    assert_eq!(
        (
            empty.active_accounts,
            empty.disabled_accounts,
            empty.retired_numbers
        ),
        (0, 0, 3)
    );
    store.close().await;
    assert!(store.operational_snapshot().await.is_err());
}

#[tokio::test]
async fn queue_gauges_follow_leases_transport_write_and_retention_without_fake_acknowledgements() {
    let (_root, store, _) = setup().await;
    let now = store.operational_snapshot().await.unwrap().sampled_at;
    let a = store
        .attach_session_if_current(UserId::new(1).unwrap(), 1, [1; 16], now)
        .await
        .unwrap();
    store.replace_privacy(&a, &[], false, now).await.unwrap();
    let message = MessageBundle {
        recipient: UserId::new(2).unwrap(),
        client_sequence: 1,
        presentation: 8,
        text: MessageText::new("PrivateMessageCanary".into()).unwrap(),
        styles: vec![],
        conference: Default::default(),
    };
    store
        .accept_message_bundle(&a, message, now - 5, &QueueLimits::default())
        .await
        .unwrap();
    let pending = store.operational_snapshot().await.unwrap();
    assert_eq!(pending.deliveries.pending, 1);
    assert!(pending.queued_payload_bytes > 0 && pending.oldest_queue_age_seconds >= 5);
    sqlx::query("UPDATE messages SET accepted_at=?")
        .bind(now + 60)
        .execute(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        store
            .operational_snapshot()
            .await
            .unwrap()
            .oldest_queue_age_seconds,
        0
    );
    sqlx::query("UPDATE messages SET accepted_at=?")
        .bind(now - 5)
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO contact_archives(owner,version,format,document) VALUES(1,1,1,'Żółć')")
        .execute(&store.pool)
        .await
        .unwrap();
    for (id, expiry) in [(1, now - 60), (2, now + 60)] {
        sqlx::query("INSERT INTO image_offers(message_id,sender,recipient,size,crc32,expires_at) VALUES(?,1,2,17,1,?)").bind(id).bind(expiry).execute(&store.pool).await.unwrap();
    }
    let storage = store.operational_snapshot().await.unwrap();
    assert_eq!(
        (
            storage.contact_archive_bytes,
            storage.unexpired_image_grants
        ),
        (8, 1)
    );
    let b = store
        .attach_session_if_current(UserId::new(2).unwrap(), 1, [2; 16], now)
        .await
        .unwrap();
    store.replace_privacy(&b, &[], false, now).await.unwrap();
    let lease = store.claim_delivery(&b, now).await.unwrap().unwrap();
    let leased = store.operational_snapshot().await.unwrap();
    assert_eq!(
        (leased.deliveries.pending, leased.deliveries.leased),
        (0, 1)
    );
    assert_eq!(leased.queued_payload_bytes, pending.queued_payload_bytes);
    store.record_transport_write(&b, &lease, now).await.unwrap();
    let written = store.operational_snapshot().await.unwrap();
    assert_eq!(
        (
            written.deliveries.written_unconfirmed,
            written.queued_payload_bytes,
            written.oldest_queue_age_seconds
        ),
        (1, 0, 0)
    );
    store.prune(now + 7 * 86400 + 1).await.unwrap();
    assert_eq!(
        store
            .operational_snapshot()
            .await
            .unwrap()
            .deliveries
            .written_unconfirmed,
        0
    );
    store.close().await;
}
