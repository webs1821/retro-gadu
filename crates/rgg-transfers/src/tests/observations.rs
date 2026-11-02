// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use super::*;

fn snapshot(images: &Images, states: [u64; 4], reserved: u64, bytes: u64) -> ImageSnapshot {
    let sample = images.image_snapshot().unwrap();
    assert_eq!(sample.transfers, states);
    assert_eq!(sample.reserved_bytes, reserved);
    assert_eq!(sample.buffered_bytes, bytes);
    assert!(sample.buffer_capacity_bytes >= bytes);
    if bytes == 0 {
        assert_eq!(sample.buffer_capacity_bytes, 0);
    }
    sample
}

#[test]
fn reservation_assembly_crc_and_output_retain_only_coordinator_owned_buffers() {
    let images = Images::default();
    let now = Instant::now();
    let data = vec![123; 5000];
    let r = route(&data);
    assert!(snapshot(&images, [0; 4], 0, 0).oldest_started.is_none());
    images.request_at(r.clone(), now).unwrap();
    assert_eq!(
        snapshot(&images, [1, 0, 0, 0], 5000, 0).oldest_started,
        Some(now)
    );
    images.next_at(&r.sender, now).unwrap();
    images
        .reply_at(&r.sender, r.recipient.user, start(&r, &data[..1800]), now)
        .unwrap();
    let partial = snapshot(&images, [0, 1, 0, 0], 5000, 1808);
    assert_eq!(partial.buffer_capacity_bytes, 5008);
    for chunk in data[1800..].chunks(1800) {
        images
            .reply_at(
                &r.sender,
                r.recipient.user,
                ImageControl::Continue {
                    image: r.image,
                    data: chunk.to_vec(),
                },
                now,
            )
            .unwrap();
    }
    snapshot(&images, [0, 0, 1, 0], 5000, 5008);
    let first_outgoing = images.next_at(&r.recipient, now).unwrap();
    // The full allocation is retained until the final chunk; the outgoing copy is excluded.
    snapshot(&images, [0, 0, 1, 0], 5000, 5008);
    while images.next_at(&r.recipient, now).is_some() {}
    assert!(snapshot(&images, [0; 4], 0, 0).oldest_started.is_none());
    assert!(matches!(first_outgoing.control, ImageControl::Start { .. }));
}

#[test]
fn reads_do_not_expire_or_refresh_and_failure_paths_release_buffers() {
    let images = Images::default();
    let now = Instant::now();
    let old = now.checked_sub(DEADLINE + Duration::from_secs(1)).unwrap();
    let r = route(b"123");
    images.request_at(r.clone(), old).unwrap();
    images.next_at(&r.sender, old).unwrap();
    images
        .reply_at(&r.sender, r.recipient.user, start(&r, b"1"), old)
        .unwrap();
    for _ in 0..20 {
        assert_eq!(
            snapshot(&images, [0, 1, 0, 0], 3, 9).oldest_started,
            Some(old)
        );
    }
    // A normal coordinator operation (not a scrape) expires the transfer at its deadline.
    assert!(images.next_at(&r.sender, now).is_none());
    assert!(
        snapshot(&images, [0, 0, 0, 1], 0, 0)
            .oldest_started
            .is_none()
    );
    images.next_at(&r.recipient, now).unwrap();
    for failure in 0..5 {
        images.request_at(r.clone(), now).unwrap();
        images.next_at(&r.sender, now).unwrap();
        images
            .reply_at(&r.sender, r.recipient.user, start(&r, b"1"), now)
            .unwrap();
        match failure {
            0 => images.cancel(&r),
            1 => images.disconnect(&r.sender),
            2 => images
                .reply_at(
                    &r.sender,
                    r.recipient.user,
                    ImageControl::Missing(r.image),
                    now,
                )
                .unwrap(),
            3 => assert_eq!(
                images
                    .reply_at(
                        &r.sender,
                        r.recipient.user,
                        ImageControl::Continue {
                            image: r.image,
                            data: b"xx".to_vec()
                        },
                        now
                    )
                    .unwrap_err(),
                MessagingError::Integrity
            ),
            _ => assert_eq!(
                images
                    .reply_at(
                        &r.sender,
                        r.recipient.user,
                        ImageControl::Continue {
                            image: r.image,
                            data: b"234".to_vec()
                        },
                        now
                    )
                    .unwrap_err(),
                MessagingError::Invalid
            ),
        }
        snapshot(&images, [0, 0, 0, 1], 0, 0);
        images.next_at(&r.recipient, now).unwrap();
        snapshot(&images, [0; 4], 0, 0);
    }
    images.request_at(r.clone(), now).unwrap();
    images.next_at(&r.sender, now).unwrap();
    images
        .reply_at(&r.sender, r.recipient.user, start(&r, b"1"), now)
        .unwrap();
    images.disconnect(&r.recipient);
    snapshot(&images, [0; 4], 0, 0);
}

#[test]
fn maximum_allocations_quotas_and_mixed_state_accounting() {
    let images = Images::default();
    let now = Instant::now();
    let mut routes = Vec::new();
    for n in 0..MAX_ACTIVE {
        let r = ImageRoute {
            sender: key(1 + 2 * n as u32),
            recipient: key(2 + 2 * n as u32),
            image: ImageId {
                size: MAX_IMAGE_BYTES,
                crc32: 0,
            },
        };
        images.request_at(r.clone(), now).unwrap();
        images.next_at(&r.sender, now).unwrap();
        images
            .reply_at(&r.sender, r.recipient.user, start(&r, b"a"), now)
            .unwrap();
        routes.push(r);
    }
    let reserved = MAX_ACTIVE as u64 * u64::from(MAX_IMAGE_BYTES);
    let full = snapshot(&images, [0, 32, 0, 0], reserved, 32 * 9);
    assert_eq!(full.buffer_capacity_bytes, reserved + 32 * 8);
    assert!(full.reserved_bytes <= MAX_BYTES as u64);
    assert_eq!(
        images.request_at(route(b"1"), now).unwrap_err(),
        MessagingError::Quota
    );
    snapshot(&images, [0, 32, 0, 0], reserved, 32 * 9);
    images.cancel(&routes[0]);
    snapshot(
        &images,
        [0, 31, 0, 1],
        31 * u64::from(MAX_IMAGE_BYTES),
        31 * 9,
    );
    for r in routes {
        images.disconnect(&r.recipient);
    }
    snapshot(&images, [0; 4], 0, 0);
}

#[test]
fn busy_or_poisoned_lock_is_unavailable_and_does_not_wait_or_fake_zero() {
    use std::sync::{Arc, mpsc};
    let images = Arc::new(Images::default());
    let held = images.items.lock().unwrap();
    let (send, recv) = mpsc::channel();
    let copy = images.clone();
    let worker = std::thread::spawn(move || send.send(copy.image_snapshot()).unwrap());
    let result = recv.recv_timeout(Duration::from_secs(1));
    drop(held);
    worker.join().unwrap();
    assert!(result.unwrap().is_none());
    assert!(images.image_snapshot().is_some());
    let copy = images.clone();
    assert!(
        std::thread::spawn(move || {
            let _held = copy.items.lock().unwrap();
            panic!("intentional test-only poison");
        })
        .join()
        .is_err()
    );
    assert!(images.image_snapshot().is_none());
}
