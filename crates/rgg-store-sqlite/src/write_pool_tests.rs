// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, database as sqlx};
use std::task::Poll;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn queued_writers_leave_readers_available_and_rollback_precedes_the_next_writer() {
    let root = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(root.path()).await.unwrap();
    let original: i64 = sqlx::query_scalar("SELECT started_at FROM runtime_state WHERE id=1")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let mut first = store.writer.begin().await.unwrap();
    sqlx::query("UPDATE runtime_state SET started_at=started_at+1 WHERE id=1")
        .execute(&mut *first)
        .await
        .unwrap();
    let mut second = Box::pin(store.writer.begin());
    let mut third = Box::pin(store.writer.begin());
    assert!(matches!(futures_util::poll!(&mut second), Poll::Pending));
    assert!(matches!(futures_util::poll!(&mut third), Poll::Pending));
    // A pending writer must not consume a reader or expose uncommitted data.
    let observed: i64 = timeout(
        Duration::from_secs(1),
        sqlx::query_scalar("SELECT started_at FROM runtime_state WHERE id=1")
            .fetch_one(&store.pool),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(observed, original);
    first.rollback().await.unwrap();
    let mut second = timeout(Duration::from_secs(1), second)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(futures_util::poll!(&mut third), Poll::Pending));
    sqlx::query("UPDATE runtime_state SET started_at=started_at+2 WHERE id=1")
        .execute(&mut *second)
        .await
        .unwrap();
    // Dropping an unfinished operation must roll back before its connection is reused.
    drop(second);
    let mut third = timeout(Duration::from_secs(1), third)
        .await
        .unwrap()
        .unwrap();
    let observed: i64 = sqlx::query_scalar("SELECT started_at FROM runtime_state WHERE id=1")
        .fetch_one(&mut *third)
        .await
        .unwrap();
    assert_eq!(observed, original);
    third.commit().await.unwrap();
    assert_eq!(store.writer.size(), 1);
    assert!(
        sqlx::query("UPDATE runtime_state SET started_at=0 WHERE id=1")
            .execute(&store.pool)
            .await
            .is_err()
    );
    store.integrity_check().await.unwrap();
    store.close().await;
}
