// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{database as sqlx, messaging_tests::setup};
use rgg_application::{conferences::ConferenceContext, messaging::*};
use rgg_domain::{MessageText, UserId};
fn uid(n: u32) -> UserId {
    UserId::new(n).unwrap()
}
fn message(others: &[u32]) -> MessageBundle {
    MessageBundle {
        recipient: uid(2),
        client_sequence: 77,
        presentation: 8,
        text: MessageText::new("context".into()).unwrap(),
        styles: vec![],
        conference: ConferenceContext::new(others.iter().map(|n| uid(*n)).collect()).unwrap(),
    }
}
#[tokio::test]
async fn context_is_validated_in_the_core_and_corrupt_metadata_is_terminal_and_purged() {
    let (_root, s, _) = setup().await;
    let a = s
        .attach_session_if_current(uid(1), 1, [1; 16], 100)
        .await
        .unwrap();
    s.replace_privacy(&a, &[], false, 100).await.unwrap();
    let b = s
        .attach_session_if_current(uid(2), 1, [2; 16], 100)
        .await
        .unwrap();
    s.replace_privacy(&b, &[], false, 100).await.unwrap();
    for others in [&[1][..], &[2][..]] {
        assert_eq!(
            s.accept_message_bundle(&a, message(others), 100, &QueueLimits::default())
                .await
                .unwrap_err(),
            MessagingError::Invalid
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM messages")
            .fetch_one(&s.pool)
            .await
            .unwrap(),
        0
    );
    for invalid in [
        Some("[0]"),
        Some("[3,3]"),
        Some("[1]"),
        Some("[2]"),
        Some("{"),
        None,
    ] {
        let id = s
            .accept_message_bundle(&a, message(&[3]), 100, &QueueLimits::default())
            .await
            .unwrap();
        sqlx::query("UPDATE messages SET conference=? WHERE id=?")
            .bind(invalid)
            .bind(id)
            .execute(&s.pool)
            .await
            .unwrap();
        assert_eq!(
            s.claim_delivery(&b, 101).await.unwrap_err(),
            MessagingError::Integrity
        );
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM messages WHERE body IS NOT NULL OR styles IS NOT NULL OR conference IS NOT NULL").fetch_one(&s.pool).await.unwrap();
        assert_eq!(count, 0);
    }
    for corruption in [
        "UPDATE messages SET sender=0 WHERE id=?",
        "UPDATE messages SET recipient=3 WHERE id=?",
    ] {
        let id = s
            .accept_message_bundle(&a, message(&[3]), 100, &QueueLimits::default())
            .await
            .unwrap();
        sqlx::query(corruption)
            .bind(id)
            .execute(&s.pool)
            .await
            .unwrap();
        assert_eq!(
            s.claim_delivery(&b, 101).await.unwrap_err(),
            MessagingError::Integrity
        );
    }
    s.accept_message_bundle(&a, message(&[3, 4]), 100, &QueueLimits::default())
        .await
        .unwrap();
    let lease = s.claim_delivery(&b, 101).await.unwrap().unwrap();
    assert_eq!(lease.conference.others(), &[uid(3), uid(4)]);
    s.record_transport_write(&b, &lease, 101).await.unwrap();
    assert!(s.claim_delivery(&b, 102).await.unwrap().is_none());
    s.close().await;
}
