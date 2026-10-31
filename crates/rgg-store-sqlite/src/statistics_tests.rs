// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx, messaging_tests::setup};
use rgg_application::{
    accounts::AccountRepository,
    messaging::MessagingRepository,
    presence::{Availability, Presence, PresenceRepository},
    statistics::StatisticsRepository,
};
use rgg_domain::UserId;

#[tokio::test]
async fn public_counters_require_current_ready_public_presence_and_exclude_tombstones() {
    let (root, store, _) = setup().await;
    let a = UserId::new(1).unwrap();
    assert_eq!(store.public_statistics().await.unwrap().registered, 3);
    let key = store
        .attach_session_if_current(a, 1, [1; 16], 100)
        .await
        .unwrap();
    assert_eq!(store.public_statistics().await.unwrap().publicly_online, 0);
    let mut p = Presence {
        availability: Availability::Available,
        description: Some("Private description".into()),
        return_at: Some(123),
        friends_only: false,
    };
    store.activate_presence(&key, &[], &p, 100).await.unwrap();
    for (availability, expected) in [
        (Availability::Available, 1),
        (Availability::Away, 1),
        (Availability::Invisible, 0),
        (Availability::Offline, 0),
    ] {
        p.availability = availability;
        store.set_presence(&key, &p).await.unwrap();
        assert_eq!(
            store.public_statistics().await.unwrap().publicly_online,
            expected
        );
    }
    p.availability = Availability::Available;
    p.friends_only = true;
    store.set_presence(&key, &p).await.unwrap();
    assert_eq!(store.public_statistics().await.unwrap().publicly_online, 0);
    p.friends_only = false;
    store.set_presence(&key, &p).await.unwrap();
    for (break_fence, restore) in [
        (
            "UPDATE accounts SET active=0 WHERE uin=1",
            "UPDATE accounts SET active=1 WHERE uin=1",
        ),
        (
            "UPDATE accounts SET session_generation=session_generation+1 WHERE uin=1",
            "UPDATE accounts SET session_generation=session_generation-1 WHERE uin=1",
        ),
        (
            "UPDATE accounts SET credential_version=credential_version+1 WHERE uin=1",
            "UPDATE accounts SET credential_version=credential_version-1 WHERE uin=1",
        ),
        (
            "UPDATE account_sessions SET ready=0 WHERE uin=1",
            "UPDATE account_sessions SET ready=1 WHERE uin=1",
        ),
        (
            "UPDATE account_presence SET session_id=zeroblob(16) WHERE uin=1",
            "UPDATE account_presence SET session_id=(SELECT session_id FROM account_sessions WHERE uin=1) WHERE uin=1",
        ),
        (
            "UPDATE account_sessions SET epoch='old' WHERE uin=1",
            "UPDATE account_sessions SET epoch=(SELECT process_epoch FROM runtime_state WHERE id=1) WHERE uin=1",
        ),
    ] {
        sqlx::query(break_fence).execute(&store.pool).await.unwrap();
        let sample = store.public_statistics().await.unwrap();
        assert_eq!((sample.registered, sample.publicly_online), (3, 0));
        sqlx::query(restore).execute(&store.pool).await.unwrap();
        assert_eq!(store.public_statistics().await.unwrap().publicly_online, 1);
    }
    store.detach_session(&key, 101).await.unwrap();
    store.retire_account(a).await.unwrap();
    let sample = store.public_statistics().await.unwrap();
    assert_eq!((sample.registered, sample.publicly_online), (2, 0));
    let b = store
        .attach_session_if_current(UserId::new(2).unwrap(), 1, [2; 16], 100)
        .await
        .unwrap();
    store.activate_presence(&b, &[], &p, 100).await.unwrap();
    assert_eq!(store.public_statistics().await.unwrap().publicly_online, 1);
    store.close().await;
    drop(store);
    let restored = SqliteStore::open(root.path()).await.unwrap();
    let sample = restored.public_statistics().await.unwrap();
    assert_eq!((sample.registered, sample.publicly_online), (2, 0));
    restored.close().await;
}
