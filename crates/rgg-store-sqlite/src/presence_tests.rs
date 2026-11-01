// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, messaging_tests::setup};
use rgg_application::{
    contacts::*,
    messaging::*,
    presence::*,
    sessions::{IoFence, SessionHub},
};
use rgg_domain::UserId;
use std::{sync::Arc, time::Duration};
fn user(n: u32) -> UserId {
    UserId::new(n).unwrap()
}
fn available() -> Presence {
    Presence {
        availability: Availability::Available,
        description: Some("Zażółć".into()),
        return_at: Some(123),
        friends_only: false,
    }
}
async fn attach(s: &SqliteStore, n: u32) -> SessionKey {
    s.attach_session_if_current(user(n), 1, [n as u8; 16], 100)
        .await
        .unwrap()
}
async fn ready(s: &SqliteStore, key: &SessionKey, rules: &[(u32, u8)], p: &Presence) {
    s.activate_presence(
        key,
        &rules
            .iter()
            .map(|(n, flags)| ContactRule {
                user: user(*n),
                flags: *flags,
            })
            .collect::<Vec<_>>(),
        p,
        100,
    )
    .await
    .unwrap();
}
async fn observed(s: &SqliteStore, b: &SessionKey) -> Presence {
    s.presence_snapshot(b).await.unwrap()[0].presence.clone()
}

#[tokio::test]
async fn effective_presence_is_ready_scoped_private_and_offline_description_survives_epoch() {
    let (root, s, _) = setup().await;
    let a = attach(&s, 1).await;
    let b = attach(&s, 2).await;
    ready(&s, &b, &[(1, 3)], &available()).await;
    assert_eq!(observed(&s, &b).await, Presence::offline());
    ready(&s, &a, &[(2, 3)], &available()).await;
    assert_eq!(observed(&s, &b).await, available());
    let mut p = available();
    p.availability = Availability::Invisible;
    s.set_presence(&a, &p).await.unwrap();
    assert_eq!(observed(&s, &b).await, Presence::offline());
    p = available();
    p.friends_only = true;
    ready(&s, &a, &[], &p).await;
    assert_eq!(observed(&s, &b).await, Presence::offline());
    s.change_contact(
        &a,
        ContactRule {
            user: user(2),
            flags: 2,
        },
        false,
        101,
    )
    .await
    .unwrap();
    assert_eq!(observed(&s, &b).await.description, p.description);
    s.change_contact(
        &a,
        ContactRule {
            user: user(2),
            flags: 4,
        },
        false,
        102,
    )
    .await
    .unwrap();
    assert_eq!(observed(&s, &b).await, Presence::offline());
    p.availability = Availability::Offline;
    ready(&s, &a, &[(2, 3)], &p).await;
    s.detach_session(&a, 103).await.unwrap();
    assert_eq!(observed(&s, &b).await.description, p.description);
    s.close().await;
    drop(s);
    let s = SqliteStore::open(root.path()).await.unwrap();
    let b = attach(&s, 2).await;
    ready(&s, &b, &[(1, 3)], &available()).await;
    assert_eq!(observed(&s, &b).await.return_at, Some(123));
    let a = attach(&s, 1).await;
    assert_eq!(observed(&s, &b).await, Presence::offline());
    ready(&s, &a, &[], &available()).await;
    s.detach_session(&a, 104).await.unwrap();
    assert_eq!(observed(&s, &b).await, Presence::offline());
    s.close().await;
}
struct TestClock;
impl Clock for TestClock {
    fn unix_seconds(&self) -> i64 {
        100
    }
}
struct Proof;
impl PasswordProof for Proof {
    fn matches(&self, p: &[u8]) -> bool {
        p == b"LocalTest9!"
    }
}
#[tokio::test]
async fn privacy_commit_waits_for_an_in_progress_publication_without_waiting_on_observer_actor() {
    let (_root, s, ring) = setup().await;
    let auth = Authenticator::new(s.clone(), ring).unwrap();
    let hub = SessionHub::new(s.clone(), Arc::new(TestClock), QueueLimits::default());
    let a = hub.account(user(1)).await.unwrap();
    let b = hub.account(user(2)).await.unwrap();
    let af = IoFence::new();
    let bf = IoFence::new();
    let ak = a
        .attach(
            auth.authenticate(user(1), &Proof).await.unwrap(),
            [1; 16],
            af.clone(),
        )
        .await
        .unwrap();
    let bk = b
        .attach(
            auth.authenticate(user(2), &Proof).await.unwrap(),
            [2; 16],
            bf.clone(),
        )
        .await
        .unwrap();
    a.privacy(ak.clone(), vec![], available()).await.unwrap();
    b.privacy(
        bk.clone(),
        vec![ContactRule {
            user: user(1),
            flags: 3,
        }],
        available(),
    )
    .await
    .unwrap();
    let publication = hub.presence_snapshot(&bk).await.unwrap();
    assert_eq!(publication.entries[0].presence, available());
    let owner = a.clone();
    let pending = tokio::spawn(async move {
        let mut p = available();
        p.availability = Availability::Invisible;
        owner.presence(ak, p).await
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!pending.is_finished());
    drop(publication);
    tokio::time::timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        hub.presence_snapshot(&bk).await.unwrap().entries[0].presence,
        Presence::offline()
    );
    // No real sockets in this fixture: signal their synthetic stopped barriers explicitly.
    af.stopped.cancel();
    bf.stopped.cancel();
    drop(a);
    drop(b);
    hub.finish_shutdown().await;
    s.close().await;
}
#[tokio::test]
async fn archive_versions_are_independent_atomic_private_and_discard_staging_on_restart() {
    let (root, s, _) = setup().await;
    let a = attach(&s, 1).await;
    let b = attach(&s, 2).await;
    ready(&s, &a, &[(2, 4)], &available()).await;
    ready(&s, &b, &[], &available()).await;
    assert_eq!(
        s.stage_archive(
            &a,
            ArchiveSegment {
                start: true,
                complete: true,
                text: "Poprzednia\r\n".into()
            }
        )
        .await
        .unwrap(),
        ArchiveResult::Published(1)
    );
    s.stage_archive(
        &a,
        ArchiveSegment {
            start: true,
            complete: false,
            text: "a".repeat(2047),
        },
    )
    .await
    .unwrap();
    assert_eq!(s.export_archive(&a).await.unwrap().text, "Poprzednia\r\n");
    assert!(s.export_archive(&b).await.unwrap().text.is_empty());
    let mut forged = a.clone();
    forged.user = user(2);
    assert!(matches!(
        s.export_archive(&forged).await,
        Err(MessagingError::Stale)
    ));
    assert_eq!(
        s.stage_archive(
            &a,
            ArchiveSegment {
                start: false,
                complete: true,
                text: "ą".into()
            }
        )
        .await
        .unwrap(),
        ArchiveResult::Published(2)
    );
    let archive = s.export_archive(&a).await.unwrap();
    assert_eq!(archive.version, 2);
    assert_eq!(archive.text.chars().count(), 2048);
    assert_eq!(
        s.accept_message_bundle(
            &b,
            MessageBundle {
                recipient: user(1),
                client_sequence: 1,
                presentation: 8,
                text: rgg_domain::MessageText::new("test".into()).unwrap(),
                styles: vec![],
                conference: Default::default(),
            },
            100,
            &QueueLimits::default()
        )
        .await
        .unwrap_err(),
        MessagingError::Rejected
    );
    s.stage_archive(
        &a,
        ArchiveSegment {
            start: true,
            complete: false,
            text: "partial".into(),
        },
    )
    .await
    .unwrap();
    s.close().await;
    drop(s);
    let s = SqliteStore::open(root.path()).await.unwrap();
    let new = attach(&s, 1).await;
    ready(&s, &new, &[], &available()).await;
    assert_eq!(s.export_archive(&new).await.unwrap().text, archive.text);
    assert_eq!(
        s.stage_archive(
            &new,
            ArchiveSegment {
                start: false,
                complete: true,
                text: "suffix".into()
            }
        )
        .await
        .unwrap_err(),
        MessagingError::Invalid
    );
    assert_eq!(
        s.stage_archive(
            &a,
            ArchiveSegment {
                start: true,
                complete: true,
                text: "old writer".into()
            }
        )
        .await
        .unwrap_err(),
        MessagingError::Stale
    );
    assert!(
        s.stage_archive(
            &new,
            ArchiveSegment {
                start: true,
                complete: true,
                text: "bad\0data".into()
            }
        )
        .await
        .is_err()
    );
    s.abort_archive(&new).await.unwrap();
    s.integrity_check().await.unwrap();
    s.close().await;
}

#[tokio::test]
async fn friends_only_status_hides_presence_without_blocking_pending_messages() {
    let (_root, s, _) = setup().await;
    let a = attach(&s, 1).await;
    let b = attach(&s, 2).await;
    ready(&s, &a, &[], &available()).await;
    ready(&s, &b, &[], &available()).await;
    s.accept_message_bundle(
        &a,
        MessageBundle {
            recipient: user(2),
            client_sequence: 7,
            presentation: 8,
            text: rgg_domain::MessageText::new("private".into()).unwrap(),
            styles: vec![],
            conference: Default::default(),
        },
        100,
        &QueueLimits::default(),
    )
    .await
    .unwrap();
    let mut restricted = available();
    restricted.friends_only = true;
    s.set_presence(&b, &restricted).await.unwrap();
    s.set_presence(&b, &available()).await.unwrap();
    let lease = s.claim_delivery(&b, 103).await.unwrap().unwrap();
    assert_eq!(lease.text.as_str(), "private");
    s.record_transport_write(&b, &lease, 104).await.unwrap();
    s.close().await;
}
