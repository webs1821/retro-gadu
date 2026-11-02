// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Bounded, ephemeral image assembly. No filesystem, network or persistent image cache.
use rgg_application::{
    images::*,
    messaging::{MessagingError, SessionKey},
};
use rgg_domain::UserId;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
const CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
const MAX_ACTIVE: usize = 32;
const MAX_PER_USER: usize = 4;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(30);
const MAX_PARTS: usize = 512;
enum State {
    Requested {
        sent: bool,
    },
    Collecting {
        filename: Vec<u8>,
        data: Vec<u8>,
        parts: usize,
    },
    Ready {
        filename: Vec<u8>,
        data: Vec<u8>,
        offset: usize,
    },
    Missing,
}
struct Transfer {
    route: ImageRoute,
    started: Instant,
    deadline: Instant,
    state: State,
}
#[derive(Default)]
pub struct Images {
    items: Mutex<Vec<Transfer>>,
}
fn expire(items: &mut [Transfer], now: Instant) {
    for item in items {
        if now >= item.deadline {
            item.state = State::Missing;
        }
    }
}
impl Images {
    fn request_at(&self, route: ImageRoute, now: Instant) -> Result<(), MessagingError> {
        route.image.validate()?;
        if route.sender.user == route.recipient.user {
            return Err(MessagingError::Invalid);
        }
        let mut items = self.items.lock().map_err(|_| MessagingError::Busy)?;
        expire(&mut items, now);
        let same = |i: &&Transfer| {
            i.route.sender.user == route.sender.user || i.route.recipient.user == route.sender.user
        };
        let receiver = |i: &&Transfer| {
            i.route.sender.user == route.recipient.user
                || i.route.recipient.user == route.recipient.user
        };
        let bytes: usize = items
            .iter()
            .filter(|i| !matches!(i.state, State::Missing))
            .map(|i| i.route.image.size as usize)
            .sum();
        if items.len() >= MAX_ACTIVE
            || items.iter().filter(same).count() >= MAX_PER_USER
            || items.iter().filter(receiver).count() >= MAX_PER_USER
            || bytes + route.image.size as usize > MAX_BYTES
            || items.iter().any(|i| {
                i.route.sender.user == route.sender.user
                    && i.route.recipient.user == route.recipient.user
                    && i.route.image == route.image
            })
        {
            return Err(MessagingError::Quota);
        }
        items.push(Transfer {
            route,
            started: now,
            deadline: now + DEADLINE,
            state: State::Requested { sent: false },
        });
        Ok(())
    }
    fn reply_at(
        &self,
        source: &SessionKey,
        recipient: UserId,
        control: ImageControl,
        now: Instant,
    ) -> Result<(), MessagingError> {
        let mut items = self.items.lock().map_err(|_| MessagingError::Busy)?;
        expire(&mut items, now);
        let item = items
            .iter_mut()
            .find(|i| {
                &i.route.sender == source
                    && i.route.recipient.user == recipient
                    && i.route.image == control.image()
            })
            .ok_or(MessagingError::Rejected)?;
        if control.validate().is_err() {
            item.state = State::Missing;
            return Err(MessagingError::Invalid);
        }
        let old = std::mem::replace(&mut item.state, State::Missing);
        let (filename, mut data, parts, chunk) = match (old, control) {
            (
                State::Requested { sent: true } | State::Collecting { .. },
                ImageControl::Missing(_),
            ) => return Ok(()),
            (
                State::Requested { sent: true },
                ImageControl::Start {
                    filename,
                    data: chunk,
                    ..
                },
            ) => (
                filename,
                Vec::with_capacity(item.route.image.size as usize),
                1,
                chunk,
            ),
            (
                State::Collecting {
                    filename,
                    data,
                    parts,
                },
                ImageControl::Continue { data: chunk, .. },
            ) => (filename, data, parts + 1, chunk),
            _ => return Err(MessagingError::Invalid),
        };
        if parts > MAX_PARTS || data.len() + chunk.len() > item.route.image.size as usize {
            return Err(MessagingError::Invalid);
        }
        data.extend_from_slice(&chunk);
        item.state = if data.len() == item.route.image.size as usize {
            if CRC.checksum(&data) != item.route.image.crc32 {
                return Err(MessagingError::Integrity);
            }
            State::Ready {
                filename,
                data,
                offset: 0,
            }
        } else {
            State::Collecting {
                filename,
                data,
                parts,
            }
        };
        Ok(())
    }
    fn next_at(&self, destination: &SessionKey, now: Instant) -> Option<ImageAction> {
        let mut items = self.items.lock().ok()?;
        expire(&mut items, now);
        let index = items.iter().position(|i| {
            (&i.route.sender == destination && matches!(i.state, State::Requested { sent: false }))
                || (&i.route.recipient == destination
                    && matches!(i.state, State::Ready { .. } | State::Missing))
        })?;
        let item = &mut items[index];
        let route = item.route.clone();
        let image = route.image;
        let mut remove = false;
        let (to_sender, control) = match &mut item.state {
            State::Requested { sent } => {
                *sent = true;
                (true, ImageControl::Request(image))
            }
            State::Missing => {
                remove = true;
                (false, ImageControl::Missing(image))
            }
            State::Ready {
                filename,
                data,
                offset,
            } => {
                let first = *offset == 0;
                let size = (MAX_CHUNK_BYTES - if first { filename.len() + 1 } else { 0 })
                    .min(data.len() - *offset);
                let chunk = data[*offset..*offset + size].to_vec();
                *offset += size;
                remove = *offset == data.len();
                (
                    false,
                    if first {
                        ImageControl::Start {
                            image,
                            filename: filename.clone(),
                            data: chunk,
                        }
                    } else {
                        ImageControl::Continue { image, data: chunk }
                    },
                )
            }
            State::Collecting { .. } => return None,
        };
        if remove {
            items.remove(index);
        }
        Some(ImageAction {
            route,
            to_sender,
            control,
        })
    }
}
impl ImageObservations for Images {
    fn image_snapshot(&self) -> Option<ImageSnapshot> {
        // Scraping must never queue behind assembly/CRC or affect transfer deadlines.
        let items = self.items.try_lock().ok()?;
        let mut sample = ImageSnapshot::default();
        for item in items.iter() {
            let (state, buffers) = match &item.state {
                State::Requested { .. } => (0, None),
                State::Collecting { filename, data, .. } => (1, Some((filename, data))),
                State::Ready { filename, data, .. } => (2, Some((filename, data))),
                State::Missing => (3, None),
            };
            sample.transfers[state] += 1;
            if state != 3 {
                sample.reserved_bytes += u64::from(item.route.image.size);
                sample.oldest_started = Some(
                    sample
                        .oldest_started
                        .map_or(item.started, |old| old.min(item.started)),
                );
            }
            if let Some((filename, data)) = buffers {
                sample.buffered_bytes += (filename.len() + data.len()) as u64;
                sample.buffer_capacity_bytes += (filename.capacity() + data.capacity()) as u64;
            }
        }
        Some(sample)
    }
}
impl ImageTransfers for Images {
    fn request(&self, route: ImageRoute) -> Result<(), MessagingError> {
        self.request_at(route, Instant::now())
    }
    fn reply(
        &self,
        source: &SessionKey,
        recipient: UserId,
        control: ImageControl,
    ) -> Result<(), MessagingError> {
        self.reply_at(source, recipient, control, Instant::now())
    }
    fn next(&self, destination: &SessionKey) -> Option<ImageAction> {
        self.next_at(destination, Instant::now())
    }
    fn cancel(&self, route: &ImageRoute) {
        if let Ok(mut items) = self.items.lock() {
            for i in items.iter_mut().filter(|i| &i.route == route) {
                i.state = State::Missing;
            }
        }
    }
    fn disconnect(&self, key: &SessionKey) {
        if let Ok(mut items) = self.items.lock() {
            items.retain(|i| &i.route.recipient != key);
            for i in items.iter_mut().filter(|i| &i.route.sender == key) {
                i.state = State::Missing;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    mod observations;
    fn key(n: u32) -> SessionKey {
        SessionKey {
            user: UserId::new(n).unwrap(),
            id: [n as u8; 16],
            generation: 1,
            credential_version: 1,
            epoch: "test".into(),
        }
    }
    fn route(data: &[u8]) -> ImageRoute {
        ImageRoute {
            sender: key(1),
            recipient: key(2),
            image: ImageId {
                size: data.len() as u32,
                crc32: CRC.checksum(data),
            },
        }
    }
    fn start(r: &ImageRoute, data: &[u8]) -> ImageControl {
        ImageControl::Start {
            image: r.image,
            filename: b"test.png".to_vec(),
            data: data.to_vec(),
        }
    }
    #[test]
    fn crc_complete_before_publication_and_bounded_repacketization() {
        assert_eq!(CRC.checksum(b"123456789"), 0xcbf43926);
        let images = Images::default();
        let data = vec![123; 5000];
        let r = route(&data);
        images.request(r.clone()).unwrap();
        assert!(images.next(&r.recipient).is_none());
        assert!(images.next(&r.sender).unwrap().to_sender);
        images
            .reply(&r.sender, r.recipient.user, start(&r, &data[..1800]))
            .unwrap();
        assert!(images.next(&r.recipient).is_none());
        for chunk in data[1800..].chunks(1800) {
            images
                .reply(
                    &r.sender,
                    r.recipient.user,
                    ImageControl::Continue {
                        image: r.image,
                        data: chunk.to_vec(),
                    },
                )
                .unwrap();
        }
        let mut result = Vec::new();
        while let Some(a) = images.next(&r.recipient) {
            a.control.validate().unwrap();
            match a.control {
                ImageControl::Start { data, .. } | ImageControl::Continue { data, .. } => {
                    result.extend(data)
                }
                _ => panic!(),
            }
        }
        assert_eq!(result, data);
        assert!(images.items.lock().unwrap().is_empty());
    }
    #[test]
    fn bad_crc_order_overflow_stale_sessions_timeout_and_disconnect_release_data() {
        for case in 0..5 {
            let images = Images::default();
            let r = route(b"123");
            images.request(r.clone()).unwrap();
            images.next(&r.sender).unwrap();
            let invalid = match case {
                0 => start(&r, b"bad"),
                1 => ImageControl::Continue {
                    image: r.image,
                    data: vec![1],
                },
                2 => start(&r, b"1234"),
                3 => ImageControl::Request(r.image),
                _ => ImageControl::Start {
                    image: r.image,
                    filename: b"../x".to_vec(),
                    data: vec![1],
                },
            };
            assert!(images.reply(&r.sender, r.recipient.user, invalid).is_err());
            assert!(matches!(
                images.next(&r.recipient).unwrap().control,
                ImageControl::Missing(_)
            ));
            assert!(images.items.lock().unwrap().is_empty());
        }
        let images = Images::default();
        let r = route(b"123");
        let now = Instant::now();
        images.request_at(r.clone(), now).unwrap();
        let mut stale = r.sender.clone();
        stale.generation += 1;
        assert!(
            images
                .reply(&stale, r.recipient.user, start(&r, b"123"))
                .is_err()
        );
        assert!(matches!(
            images
                .next_at(&r.recipient, now + DEADLINE)
                .unwrap()
                .control,
            ImageControl::Missing(_)
        ));
        images.request(r.clone()).unwrap();
        images.next(&r.sender).unwrap();
        images
            .reply(&r.sender, r.recipient.user, start(&r, b"1"))
            .unwrap();
        images.disconnect(&r.sender);
        assert!(matches!(
            images.next(&r.recipient).unwrap().control,
            ImageControl::Missing(_)
        ));
        images.request(r.clone()).unwrap();
        images.disconnect(&r.recipient);
        assert!(images.items.lock().unwrap().is_empty());
    }
    #[test]
    fn unsolicited_duplicate_and_per_user_capacity_do_not_allocate() {
        let images = Images::default();
        let r = route(b"123");
        assert!(
            images
                .reply(&r.sender, r.recipient.user, start(&r, b"123"))
                .is_err()
        );
        images.request(r.clone()).unwrap();
        assert!(images.request(r.clone()).is_err());
        for n in 3..6 {
            let mut next = r.clone();
            next.recipient = key(n);
            images.request(next).unwrap();
        }
        let mut next = r.clone();
        next.recipient = key(6);
        assert_eq!(images.request(next).unwrap_err(), MessagingError::Quota);
        images.cancel(&r);
        assert!(matches!(
            images.next(&r.recipient).unwrap().control,
            ImageControl::Missing(_)
        ));
        assert_eq!(images.items.lock().unwrap().len(), 3);
    }
    #[test]
    fn global_capacity_partial_fragment_limit_and_expiry_free_the_reservation() {
        let images = Images::default();
        let now = Instant::now();
        for n in 0..MAX_ACTIVE {
            let r = ImageRoute {
                sender: key(1 + n as u32 * 2),
                recipient: key(2 + n as u32 * 2),
                image: ImageId {
                    size: MAX_IMAGE_BYTES,
                    crc32: 0,
                },
            };
            images.request_at(r, now).unwrap();
        }
        let extra = ImageRoute {
            sender: key(100),
            recipient: key(101),
            image: ImageId { size: 1, crc32: 0 },
        };
        assert_eq!(
            images.request_at(extra.clone(), now).unwrap_err(),
            MessagingError::Quota
        );
        let first = images.items.lock().unwrap()[0].route.clone();
        images.next_at(&first.sender, now).unwrap();
        images
            .reply_at(
                &first.sender,
                first.recipient.user,
                start(&first, b"a"),
                now,
            )
            .unwrap();
        {
            let items = images.items.lock().unwrap();
            if let State::Collecting { data, .. } = &items[0].state {
                assert_eq!(data.capacity(), MAX_IMAGE_BYTES as usize);
            } else {
                panic!();
            }
        }
        images.next_at(&first.recipient, now + DEADLINE).unwrap();
        images.request_at(extra, now + DEADLINE).unwrap();
        assert!(
            images
                .items
                .lock()
                .unwrap()
                .iter()
                .all(|i| matches!(i.state, State::Missing | State::Requested { .. }))
        );
        let images = Images::default();
        let r = route(&vec![1; 1000]);
        images.request_at(r.clone(), now).unwrap();
        images.next_at(&r.sender, now).unwrap();
        images
            .reply_at(&r.sender, r.recipient.user, start(&r, &[1]), now)
            .unwrap();
        for _ in 1..MAX_PARTS {
            images
                .reply_at(
                    &r.sender,
                    r.recipient.user,
                    ImageControl::Continue {
                        image: r.image,
                        data: vec![1],
                    },
                    now,
                )
                .unwrap();
        }
        assert!(
            images
                .reply_at(
                    &r.sender,
                    r.recipient.user,
                    ImageControl::Continue {
                        image: r.image,
                        data: vec![1]
                    },
                    now
                )
                .is_err()
        );
        assert!(matches!(
            images.next_at(&r.recipient, now).unwrap().control,
            ImageControl::Missing(_)
        ));
    }
}
