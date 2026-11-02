// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx};
use rgg_application::{
    accounts::{AccountRepository, LegacyPassword, Registration},
    messaging::*,
    sessions::{IoFence, SessionHub},
};
use rgg_credentials::Keyring;
use rgg_domain::{MessageText, UserId};
use std::sync::Arc;
fn uid(n: u32) -> UserId {
    UserId::new(n).unwrap()
}
pub(super) async fn setup() -> (tempfile::TempDir, Arc<SqliteStore>, Arc<Keyring>) {
    let root = crate::test_directory();
    let store = Arc::new(SqliteStore::open(root.path()).await.unwrap());
    let ring = Arc::new(Keyring::create_new(&root.path().join("keys.toml")).unwrap());
    store.create_invite([1; 32], 3).await.unwrap();
    for n in [1, 2, 3] {
        store
            .register_with_invite(
                Registration {
                    requested_uin: Some(uid(n)),
                    invite_digest: Some([1; 32]),
                    legacy_invite_digest: None,
                    password: LegacyPassword::new(zeroize::Zeroizing::new(b"LocalTest9!".to_vec()))
                        .unwrap(),
                    email: "x@example.invalid".into(),
                },
                &*ring,
            )
            .await
            .unwrap();
    }
    (root, store, ring)
}
async fn session(store: &SqliteStore, n: u32, id: u8) -> SessionKey {
    let key = store
        .attach_session_if_current(uid(n), 1, [id; 16], 100)
        .await
        .unwrap();
    store.replace_privacy(&key, &[], false, 100).await.unwrap();
    key
}
fn message(seq: u32) -> MessageBundle {
    MessageBundle {
        recipient: uid(2),
        client_sequence: seq,
        presentation: 8,
        text: MessageText::new("Żółć".into()).unwrap(),
        styles: vec![],
        conference: Default::default(),
    }
}
async fn count(store: &SqliteStore, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(&store.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn fifo_is_enqueue_order_and_reused_sequence_is_not_deduplicated() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    let first = s
        .accept_message_bundle(&a, message(7), 100, &QueueLimits::default())
        .await
        .unwrap();
    let second = s
        .accept_message_bundle(&a, message(7), 90, &QueueLimits::default())
        .await
        .unwrap();
    assert!(second > first);
    let b = s
        .attach_session_if_current(uid(2), 1, [2; 16], 101)
        .await
        .unwrap();
    assert_eq!(
        s.claim_delivery(&b, 101).await.unwrap_err(),
        MessagingError::NotReady
    );
    s.replace_privacy(&b, &[], false, 101).await.unwrap();
    let lease = s.claim_delivery(&b, 101).await.unwrap().unwrap();
    assert_eq!(lease.id, first);
    assert!(lease.queued);
    assert!(s.claim_delivery(&b, 500).await.unwrap().is_none()); // wall-clock expiry never steals a live lease
    let mut wrong = lease.clone();
    wrong.token = [9; 16];
    assert_eq!(
        s.record_transport_write(&b, &wrong, 102).await.unwrap_err(),
        MessagingError::Stale
    );
    s.record_transport_write(&b, &lease, 102).await.unwrap();
    assert_eq!(
        s.record_transport_write(&b, &lease, 102).await.unwrap_err(),
        MessagingError::Stale
    );
    let next = s.claim_delivery(&b, 102).await.unwrap().unwrap();
    assert_eq!(next.id, second);
    assert_eq!(next.accepted_at, 90);
    s.record_transport_write(&b, &next, 103).await.unwrap();
    assert_eq!(
        count(&s, "SELECT count(*) FROM messages WHERE body IS NOT NULL").await,
        0
    );
    s.prune(103 + 7 * 86400 + 1).await.unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM messages").await, 0);
    s.close().await;
}
#[tokio::test]
async fn privacy_is_atomic_persistent_and_unblocking_does_not_resurrect_messages() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    s.accept_message_bundle(&a, message(1), 100, &QueueLimits::default())
        .await
        .unwrap();
    let b = s
        .attach_session_if_current(uid(2), 1, [2; 16], 101)
        .await
        .unwrap();
    s.replace_privacy(
        &b,
        &[ContactRule {
            user: uid(1),
            flags: 7,
        }],
        true,
        101,
    )
    .await
    .unwrap();
    assert!(s.claim_delivery(&b, 101).await.unwrap().is_none());
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM deliveries WHERE state='RejectedByPrivacy'"
        )
        .await,
        1
    );
    assert_eq!(
        s.accept_message_bundle(&a, message(2), 102, &QueueLimits::default())
            .await
            .unwrap_err(),
        MessagingError::Rejected
    );
    s.change_contact(
        &b,
        ContactRule {
            user: uid(1),
            flags: 1,
        },
        true,
        102,
    )
    .await
    .unwrap();
    assert_eq!(
        count(&s, "SELECT flags FROM contact_rules WHERE owner=2").await,
        6
    );
    s.change_contact(
        &b,
        ContactRule {
            user: uid(1),
            flags: 4,
        },
        true,
        102,
    )
    .await
    .unwrap();
    assert!(s.claim_delivery(&b, 102).await.unwrap().is_none());
    s.accept_message_bundle(&a, message(3), 103, &QueueLimits::default())
        .await
        .unwrap();
    assert!(s.claim_delivery(&b, 103).await.unwrap().is_some());
    s.close().await;
}
#[tokio::test]
async fn quotas_expiry_and_terminal_payload_retention_are_enforced() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    let q = QueueLimits {
        per_recipient_messages: 1,
        ttl_seconds: 5,
        ..QueueLimits::default()
    };
    s.accept_message_bundle(&a, message(1), 100, &q)
        .await
        .unwrap();
    assert_eq!(
        s.accept_message_bundle(&a, message(2), 101, &q)
            .await
            .unwrap_err(),
        MessagingError::Quota
    );
    assert_eq!(count(&s, "SELECT count(*) FROM messages").await, 1);
    s.prune(105).await.unwrap();
    assert_eq!(
        count(&s, "SELECT count(*) FROM deliveries WHERE state='Expired'").await,
        1
    );
    assert_eq!(
        count(&s, "SELECT count(*) FROM messages WHERE body IS NOT NULL").await,
        0
    );
    s.accept_message_bundle(&a, message(2), 105, &q)
        .await
        .unwrap();
    let b = session(&s, 2, 2).await;
    let lease = s.claim_delivery(&b, 106).await.unwrap().unwrap();
    s.prune(1000000).await.unwrap();
    assert_eq!(
        count(&s, "SELECT count(*) FROM deliveries WHERE state='Leased'").await,
        1
    );
    s.release_delivery(&b, &lease, 107).await.unwrap();
    let again = s.claim_delivery(&b, 107).await.unwrap().unwrap();
    assert_ne!(again.token, lease.token);
    assert_eq!(
        s.record_transport_write(&b, &lease, 107).await.unwrap_err(),
        MessagingError::Stale
    );
    s.record_transport_write(&b, &again, 108).await.unwrap();
    s.close().await;
}
#[tokio::test]
async fn new_epoch_recovers_leases_and_stale_sessions_cannot_write() {
    let (root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    let b = session(&s, 2, 2).await;
    s.accept_message_bundle(&a, message(1), 100, &QueueLimits::default())
        .await
        .unwrap();
    let lease = s.claim_delivery(&b, 101).await.unwrap().unwrap();
    let old_epoch = b.epoch.clone();
    s.close().await;
    drop(s);
    let s = SqliteStore::open(root.path()).await.unwrap();
    assert_ne!(s.epoch, old_epoch);
    assert_eq!(count(&s, "SELECT count(*) FROM account_sessions").await, 0);
    assert_eq!(
        s.record_transport_write(&b, &lease, 102).await.unwrap_err(),
        MessagingError::Stale
    );
    let new = session(&s, 2, 3).await;
    assert!(new.generation > b.generation);
    assert_eq!(
        s.detach_session(&b, 102).await.unwrap_err(),
        MessagingError::Stale
    );
    assert!(s.session_is_current(&new).await.unwrap());
    let retry = s.claim_delivery(&new, 102).await.unwrap().unwrap();
    assert_eq!(retry.id, lease.id);
    assert_ne!(retry.token, lease.token);
    assert!(retry.queued);
    s.record_transport_write(&new, &retry, 103).await.unwrap();
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM delivery_attempts WHERE outcome='RecoveredEpoch'"
        )
        .await,
        1
    );
    s.integrity_check().await.unwrap();
    s.close().await;
}
#[tokio::test]
async fn full_history_scrubs_sparse_payloads_without_expiring_a_live_writer() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    let b = session(&s, 2, 2).await;
    let first = s
        .accept_message_bundle(&a, message(1), 100, &QueueLimits::default())
        .await
        .unwrap();
    let second = s
        .accept_message_bundle(&a, message(2), 100, &QueueLimits::default())
        .await
        .unwrap();
    let lease = s.claim_delivery(&b, 101).await.unwrap().unwrap();
    assert_eq!(lease.id, first);
    let mut tx = s.writer.begin().await.unwrap();
    sqlx::query("WITH RECURSIVE n(id) AS (VALUES(100) UNION ALL SELECT id+1 FROM n WHERE id<100104) INSERT INTO messages(id,sender,recipient,client_sequence,payload_version,presentation,body,styles,conference,payload_bytes,accepted_at,expires_at) SELECT id,1,2,id,1,8,CASE WHEN id=100102 THEN 'private text' END,CASE WHEN id=100103 THEN '[]' END,CASE WHEN id=100104 THEN '[3]' END,16,100,200 FROM n").execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO deliveries(message_id,recipient,enqueue_order,state,terminal_at) SELECT id,2,id,'TransportWrittenUnconfirmed',CASE WHEN id=100 THEN 1000-7*86400-1 ELSE 990 END FROM messages WHERE id>=100").execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    s.prune(1000).await.unwrap();
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM deliveries WHERE terminal_at IS NOT NULL"
        )
        .await,
        100000
    );
    assert_eq!(
        count(
            &s,
            "SELECT min(message_id) FROM deliveries WHERE terminal_at IS NOT NULL"
        )
        .await,
        105
    );
    assert_eq!(count(&s, "SELECT count(*) FROM messages WHERE id>=100 AND (body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL)").await, 0);
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM messages WHERE id<100 AND body IS NOT NULL"
        )
        .await,
        2
    );
    assert_eq!(
        count(&s, "SELECT count(*) FROM deliveries WHERE state='Leased'").await,
        1
    );

    // An abrupt UTC jump may expire Pending and history, never a live writer.
    let future = 101 + i64::from(QueueLimits::default().ttl_seconds);
    s.prune(future).await.unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM deliveries").await, 2);
    assert_eq!(
        count(&s, "SELECT count(*) FROM deliveries WHERE state='Expired'").await,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT message_id FROM deliveries WHERE state='Expired'")
            .fetch_one(&s.pool)
            .await
            .unwrap(),
        second
    );
    assert_eq!(count(&s, "SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL").await, 1);
    // Returning UTC cannot resurrect the expired entry, and the original lease remains valid.
    s.record_transport_write(&b, &lease, 102).await.unwrap();
    assert!(s.claim_delivery(&b, 103).await.unwrap().is_none());
    assert_eq!(count(&s, "SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL").await, 0);
    s.integrity_check().await.unwrap();
    s.close().await;
}

struct TestClock;
#[tokio::test]
async fn concurrent_acceptance_obeys_atomic_recipient_quota_and_retirement_purges_payloads() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    let mut tasks = tokio::task::JoinSet::new();
    for n in 0..12 {
        let s = s.clone();
        let a = a.clone();
        tasks.spawn(async move {
            s.accept_message_bundle(
                &a,
                message(n),
                100,
                &QueueLimits {
                    per_recipient_messages: 3,
                    ..QueueLimits::default()
                },
            )
            .await
        });
    }
    let mut accepted = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(_) => accepted += 1,
            Err(e) => assert_eq!(e, MessagingError::Quota),
        }
    }
    assert_eq!(accepted, 3);
    assert_eq!(count(&s, "SELECT count(*) FROM messages").await, 3);
    s.retire_account(uid(2)).await.unwrap();
    assert_eq!(
        count(&s, "SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL").await,
        0
    );
    assert_eq!(
        s.accept_message_bundle(&a, message(13), 101, &QueueLimits::default())
            .await
            .unwrap_err(),
        MessagingError::Rejected
    );
    s.close().await;
}
#[tokio::test]
async fn corrupt_payload_is_terminal_and_attempt_history_is_bounded() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1, 1).await;
    let b = session(&s, 2, 2).await;
    s.accept_message_bundle(&a, message(1), 100, &QueueLimits::default())
        .await
        .unwrap();
    assert!(sqlx::query("UPDATE deliveries SET state='Leased',session_id=zeroblob(16),epoch='x',generation=1,lease_until=120").execute(&s.writer).await.is_err());
    assert!(
        sqlx::query("UPDATE deliveries SET state='Expired'")
            .execute(&s.writer)
            .await
            .is_err()
    );
    for _ in 0..40 {
        let lease = s.claim_delivery(&b, 101).await.unwrap().unwrap();
        s.release_delivery(&b, &lease, 101).await.unwrap();
    }
    assert_eq!(
        count(&s, "SELECT count(*) FROM delivery_attempts").await,
        32
    );
    sqlx::query("UPDATE messages SET styles='invalid-json'")
        .execute(&s.writer)
        .await
        .unwrap();
    assert_eq!(
        s.claim_delivery(&b, 102).await.unwrap_err(),
        MessagingError::Integrity
    );
    assert_eq!(
        count(&s, "SELECT count(*) FROM deliveries WHERE state='Rejected'").await,
        1
    );
    assert!(s.claim_delivery(&b, 103).await.unwrap().is_none());
    s.close().await;
}
impl Clock for TestClock {
    fn unix_seconds(&self) -> i64 {
        100
    }
}
struct GoodProof;
impl PasswordProof for GoodProof {
    fn matches(&self, p: &[u8]) -> bool {
        p == b"LocalTest9!"
    }
}
#[tokio::test]
async fn actor_waits_for_socket_barrier_and_revocation_fences_previously_verified_login() {
    let (_root, s, ring) = setup().await;
    let auth = Authenticator::new(s.clone(), ring).unwrap();
    let hub = SessionHub::new(s.clone(), Arc::new(TestClock), QueueLimits::default());
    let actor = hub.account(uid(1)).await.unwrap();
    let f1 = IoFence::new();
    let old = actor
        .attach(
            auth.authenticate(uid(1), &GoodProof).await.unwrap(),
            [1; 16],
            f1.clone(),
        )
        .await
        .unwrap();
    let identity = auth.authenticate(uid(1), &GoodProof).await.unwrap();
    let stale = auth.authenticate(uid(1), &GoodProof).await.unwrap();
    let f2 = IoFence::new();
    let other = actor.clone();
    let fence = f2.clone();
    let next = tokio::spawn(async move { other.attach(identity, [2; 16], fence).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), f1.stop.cancelled())
        .await
        .unwrap();
    assert!(!next.is_finished());
    assert!(s.session_is_current(&old).await.unwrap());
    f1.stopped.cancel();
    let new = next.await.unwrap().unwrap();
    actor.closed(old.id).await.unwrap();
    assert!(s.session_is_current(&new).await.unwrap());
    let other = actor.clone();
    let revoke = tokio::spawn(async move { other.revoke_credentials(1).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), f2.stop.cancelled())
        .await
        .unwrap();
    assert!(!revoke.is_finished());
    f2.stopped.cancel();
    revoke.await.unwrap().unwrap();
    assert_eq!(
        actor
            .attach(stale, [3; 16], IoFence::new())
            .await
            .unwrap_err(),
        MessagingError::Stale
    );
    assert_eq!(
        s.accept_message_bundle(&old, message(1), 100, &QueueLimits::default())
            .await
            .unwrap_err(),
        MessagingError::Stale
    );
    assert!(auth.authenticate(uid(999), &GoodProof).await.is_err());
    drop(actor);
    hub.finish_shutdown().await;
    s.close().await;
}
