// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{database as sqlx, messaging_tests::setup};
use rgg_application::{accounts::AccountRepository, images::*, messaging::*};
use rgg_domain::{MessageText, UserId};
fn uid(n: u32) -> UserId {
    UserId::new(n).unwrap()
}
fn image() -> ImageId {
    ImageId {
        size: 2048,
        crc32: 123,
    }
}
fn message() -> MessageBundle {
    MessageBundle {
        recipient: uid(2),
        client_sequence: 7,
        presentation: 8,
        text: MessageText::new("\u{a0}".into()).unwrap(),
        styles: vec![TextStyle {
            position: 0,
            bold: false,
            italic: false,
            underline: false,
            color: None,
            image: Some(image()),
        }],
        conference: Default::default(),
    }
}
async fn session(s: &crate::SqliteStore, n: u32) -> SessionKey {
    let k = s
        .attach_session_if_current(uid(n), 1, [n as u8; 16], 100)
        .await
        .unwrap();
    s.replace_privacy(&k, &[], false, 100).await.unwrap();
    s.set_image_limit(&k, 255).await.unwrap();
    k
}
async fn count(s: &crate::SqliteStore) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM image_offers")
        .fetch_one(&s.pool)
        .await
        .unwrap()
}
#[tokio::test]
async fn offers_are_atomic_recipient_scoped_and_outlive_payload_and_history() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1).await;
    let b = session(&s, 2).await;
    let c = session(&s, 3).await;
    assert!(s.authorize_image(&b, uid(1), image(), 100).await.is_err());
    let limits = QueueLimits {
        per_recipient_messages: 0,
        ..Default::default()
    };
    assert_eq!(
        s.accept_message_bundle(&a, message(), 100, &limits)
            .await
            .unwrap_err(),
        MessagingError::Quota
    );
    assert_eq!(count(&s).await, 0);
    let id = s
        .accept_message_bundle(&a, message(), 100, &QueueLimits::default())
        .await
        .unwrap();
    assert_eq!(count(&s).await, 1);
    assert!(s.authorize_image(&c, uid(1), image(), 100).await.is_err());
    s.set_image_limit(&b, 1).await.unwrap();
    assert!(s.authorize_image(&b, uid(1), image(), 100).await.is_err());
    s.set_image_limit(&b, 2).await.unwrap();
    let route = s.authorize_image(&b, uid(1), image(), 100).await.unwrap();
    assert_eq!(route.sender, a);
    let lease = s.claim_delivery(&b, 101).await.unwrap().unwrap();
    assert_eq!(lease.styles[0].image, Some(image()));
    s.record_transport_write(&b, &lease, 101).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL"
        )
        .fetch_one(&s.pool)
        .await
        .unwrap(),
        0
    );
    sqlx::query("DELETE FROM messages WHERE id=?")
        .bind(id)
        .execute(&s.pool)
        .await
        .unwrap();
    assert!(s.image_route_current(&route, 102).await.unwrap());
    assert!(
        !s.image_route_current(&route, 101 + OFFER_RETENTION_SECONDS)
            .await
            .unwrap()
    );
    s.prune(101 + OFFER_RETENTION_SECONDS).await.unwrap();
    assert_eq!(count(&s).await, 0);
    s.close().await;
}
#[tokio::test]
async fn blocks_revoke_permanently_sessions_are_fenced_and_retirement_cascades() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1).await;
    let b = session(&s, 2).await;
    s.accept_message_bundle(&a, message(), 100, &QueueLimits::default())
        .await
        .unwrap();
    let route = s.authorize_image(&b, uid(1), image(), 100).await.unwrap();
    s.change_contact(
        &a,
        ContactRule {
            user: uid(2),
            flags: 4,
        },
        false,
        101,
    )
    .await
    .unwrap();
    assert_eq!(count(&s).await, 0);
    assert!(!s.image_route_current(&route, 101).await.unwrap());
    s.change_contact(
        &a,
        ContactRule {
            user: uid(2),
            flags: 4,
        },
        true,
        102,
    )
    .await
    .unwrap();
    assert!(s.authorize_image(&b, uid(1), image(), 102).await.is_err());
    s.accept_message_bundle(&a, message(), 103, &QueueLimits::default())
        .await
        .unwrap();
    let newer = s
        .attach_session_if_current(uid(1), 1, [9; 16], 104)
        .await
        .unwrap();
    assert!(!s.image_route_current(&route, 104).await.unwrap());
    s.replace_privacy(&newer, &[], false, 104).await.unwrap();
    assert_ne!(
        s.authorize_image(&b, uid(1), image(), 104)
            .await
            .unwrap()
            .sender,
        route.sender
    );
    s.begin_initial_state(&b).await.unwrap();
    assert!(s.authorize_image(&b, uid(1), image(), 105).await.is_err());
    s.replace_privacy(&b, &[], false, 105).await.unwrap();
    s.detach_session(&newer, 106).await.unwrap();
    s.retire_account(uid(1)).await.unwrap();
    assert_eq!(count(&s).await, 0);
    s.close().await;
}
#[tokio::test]
async fn retained_offer_quota_cannot_be_bypassed_by_delivering_or_deleting_text() {
    let (_root, s, _) = setup().await;
    let a = session(&s, 1).await;
    sqlx::query("WITH RECURSIVE n(v) AS (SELECT 1 UNION ALL SELECT v+1 FROM n WHERE v<8000) INSERT INTO image_offers(message_id,sender,recipient,size,crc32,expires_at) SELECT -v,1,2,1,v,1000 FROM n").execute(&s.pool).await.unwrap();
    assert_eq!(
        s.accept_message_bundle(&a, message(), 100, &QueueLimits::default())
            .await
            .unwrap_err(),
        MessagingError::Quota
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM messages")
            .fetch_one(&s.pool)
            .await
            .unwrap(),
        0
    );
    s.accept_message_bundle(&a, message(), 1001, &QueueLimits::default())
        .await
        .unwrap();
    assert_eq!(count(&s).await, 1);
    s.close().await;
}
#[tokio::test]
async fn image_publication_serializes_block_commit_without_awaiting_an_actor() {
    use rgg_application::{
        presence::Presence,
        sessions::{IoFence, SessionHub},
    };
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    struct Now;
    impl Clock for Now {
        fn unix_seconds(&self) -> i64 {
            100
        }
    }
    struct Proof;
    impl PasswordProof for Proof {
        fn matches(&self, password: &[u8]) -> bool {
            password == b"LocalTest9!"
        }
    }
    #[derive(Default)]
    struct TestPort(Mutex<Option<ImageRoute>>);
    impl ImageTransfers for TestPort {
        fn request(&self, r: ImageRoute) -> Result<(), MessagingError> {
            *self.0.lock().unwrap() = Some(r);
            Ok(())
        }
        fn next(&self, _: &SessionKey) -> Option<ImageAction> {
            self.0.lock().unwrap().take().map(|r| ImageAction {
                control: ImageControl::Request(r.image),
                route: r,
                to_sender: true,
            })
        }
        fn reply(&self, _: &SessionKey, _: UserId, _: ImageControl) -> Result<(), MessagingError> {
            unreachable!()
        }
        fn cancel(&self, _: &ImageRoute) {}
        fn disconnect(&self, _: &SessionKey) {}
    }
    let (_root, s, ring) = setup().await;
    let auth = Authenticator::new(s.clone(), ring).unwrap();
    let hub = SessionHub::new(s.clone(), Arc::new(Now), QueueLimits::default());
    let a = hub.account(uid(1)).await.unwrap();
    let b = hub.account(uid(2)).await.unwrap();
    let af = IoFence::new();
    let bf = IoFence::new();
    let ak = a
        .attach(
            auth.authenticate(uid(1), &Proof).await.unwrap(),
            [1; 16],
            af.clone(),
        )
        .await
        .unwrap();
    let bk = b
        .attach(
            auth.authenticate(uid(2), &Proof).await.unwrap(),
            [2; 16],
            bf.clone(),
        )
        .await
        .unwrap();
    a.privacy(ak.clone(), vec![], Presence::offline())
        .await
        .unwrap();
    b.privacy(bk.clone(), vec![], Presence::offline())
        .await
        .unwrap();
    hub.set_image_limit(&bk, 255).await.unwrap();
    a.accept(ak.clone(), message()).await.unwrap();
    let port = TestPort::default();
    drop(
        hub.image_command(&bk, uid(1), ImageControl::Request(image()), &port)
            .await
            .unwrap(),
    );
    let publication = hub.next_image(&ak, &port).await.unwrap();
    assert!(publication.response.is_some());
    let owner = a.clone();
    let pending = tokio::spawn(async move {
        owner
            .contact(
                ak,
                ContactRule {
                    user: uid(2),
                    flags: 4,
                },
                false,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!pending.is_finished());
    drop(publication);
    tokio::time::timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(count(&s).await, 0);
    let denied = hub
        .image_command(&bk, uid(1), ImageControl::Request(image()), &port)
        .await
        .unwrap();
    assert_eq!(denied.status, 6);
    drop(denied);
    af.stopped.cancel();
    bf.stopped.cancel();
    drop(a);
    drop(b);
    hub.finish_shutdown().await;
    s.close().await;
}
