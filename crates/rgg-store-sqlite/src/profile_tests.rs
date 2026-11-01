// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{SqliteStore, messaging_tests::setup};
use rgg_application::{
    accounts::{AccountRepository, LegacyPassword, Registration},
    directory::*,
    messaging::*,
};
use rgg_domain::UserId;
const NOW: i64 = 1793491200;
fn user(n: u32) -> UserId {
    UserId::new(n).unwrap()
}
async fn key(s: &SqliteStore, n: u32) -> SessionKey {
    let k = s
        .attach_session_if_current(user(n), 1, [n as u8; 16], NOW)
        .await
        .unwrap();
    s.replace_privacy(&k, &[], false, NOW).await.unwrap();
    k
}
async fn own(s: &SqliteStore, k: &SessionKey) -> Profile {
    let ReadResult::Own(p) = s.read_directory(k, Read::Own, NOW).await.unwrap() else {
        panic!()
    };
    p
}
async fn search(s: &SqliteStore, k: &SessionKey, q: Search) -> Page {
    let ReadResult::Page(p) = s.read_directory(k, Read::Search(q), NOW).await.unwrap() else {
        panic!()
    };
    p
}
#[tokio::test]
async fn profiles_are_opt_in_fenced_replace_whole_form_and_use_literal_polish_filters() {
    let (_root, s, _) = setup().await;
    let a = key(&s, 1).await;
    let b = key(&s, 2).await;
    assert!(search(&s, &b, Search::default()).await.people.is_empty());
    s.replace_profile(
        &a,
        Profile {
            first_name: "Łucja".into(),
            last_name: "Kowalska".into(),
            nickname: "Żółwik_%'".into(),
            city: "Łódź".into(),
            family_name: "Nowak".into(),
            family_city: "Gdańsk".into(),
            birth_year: Some(1984),
            gender: Some(Gender::Female),
        },
        NOW,
    )
    .await
    .unwrap();
    let q = Search {
        first_name: "luCJa".into(),
        last_name: "NOWAK".into(),
        city: "gdansk".into(),
        nickname: "_%'".into(),
        gender: Some(Gender::Female),
        years: Some((1984, 1984)),
        ..Default::default()
    };
    assert_eq!(search(&s, &b, q.clone()).await.people.len(), 1);
    assert!(
        search(
            &s,
            &b,
            Search {
                gender: Some(Gender::Male),
                ..q
            }
        )
        .await
        .people
        .is_empty()
    );
    let mut forged = a.clone();
    forged.user = user(2);
    assert_eq!(
        s.replace_profile(&forged, Profile::default(), NOW).await,
        Err(MessagingError::Stale)
    );
    let invalid = Profile {
        birth_year: Some(2027),
        ..Default::default()
    };
    assert_eq!(
        s.replace_profile(&a, invalid, NOW).await,
        Err(MessagingError::Invalid)
    );
    assert_eq!(own(&s, &a).await.first_name, "Łucja");
    s.replace_profile(
        &a,
        Profile {
            nickname: "new".into(),
            ..Default::default()
        },
        NOW,
    )
    .await
    .unwrap();
    let p = own(&s, &a).await;
    assert!(p.first_name.is_empty() && p.gender.is_none() && p.birth_year.is_none());
    s.replace_profile(
        &a,
        Profile {
            gender: Some(Gender::Female),
            ..Default::default()
        },
        NOW,
    )
    .await
    .unwrap();
    assert!(search(&s, &b, Search::default()).await.people.is_empty());
    let replacement = key(&s, 1).await;
    assert!(s.read_directory(&a, Read::Own, NOW).await.is_err());
    assert_eq!(
        s.replace_profile(&a, Profile::default(), NOW).await,
        Err(MessagingError::Stale)
    );
    assert!(own(&s, &replacement).await.is_empty());
    s.close().await;
}
#[tokio::test]
async fn keyset_paging_is_ordered_and_retirement_cascades_profiles() {
    let (_root, s, ring) = setup().await;
    s.create_invite([9; 32], 50).await.unwrap();
    for n in 4..45 {
        s.register_with_invite(
            Registration {
                requested_uin: Some(user(n)),
                invite_digest: Some([9; 32]),
                legacy_invite_digest: None,
                password: LegacyPassword::new(zeroize::Zeroizing::new(b"LocalTest9!".to_vec()))
                    .unwrap(),
                email: "x@example.invalid".into(),
            },
            &*ring,
        )
        .await
        .unwrap();
        let k = key(&s, n).await;
        s.replace_profile(
            &k,
            Profile {
                nickname: "page".into(),
                ..Default::default()
            },
            NOW,
        )
        .await
        .unwrap();
    }
    let viewer = key(&s, 1).await;
    let mut next = 0;
    let mut found = vec![];
    let mut sizes = vec![];
    loop {
        let page = search(
            &s,
            &viewer,
            Search {
                nickname: "page".into(),
                start: next,
                ..Default::default()
            },
        )
        .await;
        sizes.push(page.people.len());
        found.extend(page.people.iter().map(|p| p.user.get()));
        next = page.next_start;
        if next == 0 {
            break;
        }
    }
    assert_eq!(sizes, vec![20, 20, 1]);
    assert_eq!(found, (4..45).collect::<Vec<_>>());
    assert!(
        search(
            &s,
            &viewer,
            Search {
                active_only: true,
                ..Default::default()
            }
        )
        .await
        .people
        .is_empty()
    );
    let retiring = key(&s, 44).await;
    s.detach_session(&retiring, NOW).await.unwrap();
    s.retire_account(user(44)).await.unwrap();
    assert!(
        search(
            &s,
            &viewer,
            Search {
                user: Some(user(44)),
                ..Default::default()
            }
        )
        .await
        .people
        .is_empty()
    );
    s.close().await;
}
#[tokio::test]
async fn directory_rate_limit_survives_session_replacement_restart_and_backwards_clock() {
    let (root, s, _) = setup().await;
    let a = key(&s, 1).await;
    for _ in 0..20 {
        s.read_directory(&a, Read::Own, NOW).await.unwrap();
    }
    assert!(matches!(
        s.read_directory(&a, Read::Own, NOW).await,
        Err(MessagingError::Quota)
    ));
    let a = key(&s, 1).await;
    assert!(matches!(
        s.read_directory(&a, Read::Own, NOW - 5).await,
        Err(MessagingError::Quota)
    ));
    s.close().await;
    drop(s);
    let s = SqliteStore::open(root.path()).await.unwrap();
    let a = key(&s, 1).await;
    assert!(matches!(
        s.read_directory(&a, Read::Own, NOW).await,
        Err(MessagingError::Quota)
    ));
    assert!(s.read_directory(&a, Read::Own, NOW + 10).await.is_ok());
    s.close().await;
}

struct TestClock;
impl Clock for TestClock {
    fn unix_seconds(&self) -> i64 {
        NOW
    }
}
struct Proof;
impl PasswordProof for Proof {
    fn matches(&self, password: &[u8]) -> bool {
        password == b"LocalTest9!"
    }
}
#[tokio::test]
async fn removing_a_profile_waits_for_directory_publication_to_finish() {
    use rgg_application::{
        presence::Presence,
        sessions::{IoFence, SessionHub},
    };
    use std::{sync::Arc, time::Duration};
    let (_root, s, ring) = setup().await;
    let auth = Authenticator::new(s.clone(), ring).unwrap();
    let hub = SessionHub::new(s.clone(), Arc::new(TestClock), QueueLimits::default());
    let actor = hub.account(user(1)).await.unwrap();
    let fence = IoFence::new();
    let k = actor
        .attach(
            auth.authenticate(user(1), &Proof).await.unwrap(),
            [1; 16],
            fence.clone(),
        )
        .await
        .unwrap();
    actor
        .privacy(k.clone(), vec![], Presence::offline())
        .await
        .unwrap();
    actor
        .profile(
            k.clone(),
            Profile {
                nickname: "public".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let publication = hub
        .directory(&k, Read::Search(Search::default()))
        .await
        .unwrap();
    let ReadResult::Page(ref page) = publication.result else {
        panic!()
    };
    assert_eq!(page.people.len(), 1);
    let owner = actor.clone();
    let old = k.clone();
    let pending = tokio::spawn(async move { owner.profile(old, Profile::default()).await });
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!pending.is_finished());
    drop(publication);
    tokio::time::timeout(Duration::from_secs(2), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let publication = hub
        .directory(&k, Read::Search(Search::default()))
        .await
        .unwrap();
    let ReadResult::Page(ref page) = publication.result else {
        panic!()
    };
    assert!(page.people.is_empty());
    drop(publication);
    fence.stopped.cancel();
    drop(actor);
    hub.finish_shutdown().await;
    s.close().await;
}
