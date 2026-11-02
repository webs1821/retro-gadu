// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Own synthetic corpus/mutator; wire facts: libgadu-wire-reference in THIRD_PARTY.toml.
//! Deterministic mutation testing, without coverage guidance or native-client claims.
use bytes::BytesMut;
use rgg_application::messaging::DeliveryLease;
use rgg_domain::UserId;
use rgg_protocol::{
    frame, gg6, gg6_contacts, gg6_directory, gg6_images, gg6_messages, gg6_presence,
};
use serde_json::{Value, json};

const TARGETS: [&str; 7] = [
    "login",
    "status",
    "contacts",
    "archive",
    "directory",
    "message",
    "image",
];
const INPUT_LIMIT: usize = 8192;

fn bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2));
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn corpus() -> Vec<(String, String, Vec<u8>, bool)> {
    let value: Value = serde_json::from_str(include_str!("fixtures/wire-corpus.json")).unwrap();
    assert_eq!(value["format"], 1);
    value["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            (
                case["name"].as_str().unwrap().into(),
                case["target"].as_str().unwrap().into(),
                bytes(case["hex"].as_str().unwrap()),
                case["accepted"].as_bool().unwrap(),
            )
        })
        .collect()
}
fn user(value: u32) {
    assert!((1..=0xff_ffff).contains(&value));
}

/// Check successful output contracts as well as refusal of malformed input.
fn inspect(target: &str, body: &[u8]) -> bool {
    match target {
        "login" => {
            let Ok(value) = gg6::decode_login(body) else {
                return false;
            };
            user(value.user.get());
            assert!((31..=106).contains(&body.len()));
            if let Some(text) = &value.description {
                let encoded = gg6::encode_text(text).unwrap();
                assert!(encoded.len() <= 70);
                assert_eq!(&encoded[..], &body[31..31 + encoded.len()]);
            }
            if value.return_at.is_some() {
                assert!(value.description.is_some());
            }
            let _ = gg6_presence::from_login(&value); // status policy is checked after LOGIN60 decoding
        }
        "status" => {
            let Ok(value) = gg6_presence::decode_status(body) else {
                return false;
            };
            assert!((4..=79).contains(&body.len()));
            if let Some(text) = value.description {
                let encoded = gg6::encode_text(&text).unwrap();
                assert!(encoded.len() <= 70);
                assert_eq!(&encoded[..], &body[4..4 + encoded.len()]);
            }
        }
        "contacts" => {
            let Ok(values) = gg6_messages::contacts(body) else {
                return false;
            };
            assert!(values.len() <= 400);
            assert_eq!(values.len() * 5, body.len());
            for (value, raw) in values.iter().zip(body.as_chunks::<5>().0.iter()) {
                user(value.user.get());
                assert_eq!(value.user.get().to_le_bytes(), raw[..4]);
                assert_eq!(value.flags, raw[4]);
                assert_eq!(value.flags & !7, 0);
            }
        }
        "archive" => {
            let Ok(value) = gg6_contacts::decode(body) else {
                return false;
            };
            match value {
                gg6_contacts::Request::Get => assert_eq!(body, &[2]),
                gg6_contacts::Request::Put(segment) => {
                    assert_eq!(&gg6::encode_text(&segment.text).unwrap()[..], &body[1..]);
                    assert!(body.len() <= 2048);
                    assert_eq!(segment.start, body[0] == 0);
                    assert_eq!(segment.complete, body.len() < 2048);
                }
            }
        }
        "directory" => {
            let Ok(value) = gg6_directory::decode(body) else {
                return false;
            };
            assert!(body.len() <= 4096);
            assert_eq!(
                gg6_directory::header(body),
                Some((value.kind, value.sequence))
            );
            assert!((1..=3).contains(&value.kind));
            if value.kind == 3 {
                assert_ne!(value.sequence, 0);
            }
        }
        "message" => {
            let Ok(value) = gg6_messages::decode_message(body) else {
                return false;
            };
            user(value.recipient.get());
            let encoded = gg6::encode_text(value.text.as_str()).unwrap();
            assert!((1..=2000).contains(&encoded.len()));
            assert_eq!(&encoded[..], &body[12..12 + encoded.len()]);
            assert!(value.styles.len() <= 256 && value.conference.others().len() <= 32);
            assert!(
                value
                    .styles
                    .iter()
                    .all(|style| usize::from(style.position) <= encoded.len())
            );
            // A sender outside this synthetic conference makes an accepted bundle representable.
            let sender = (1..=35)
                .find(|candidate| {
                    *candidate != value.recipient.get()
                        && !value
                            .conference
                            .others()
                            .iter()
                            .any(|other| other.get() == *candidate)
                })
                .unwrap();
            let lease = DeliveryLease {
                id: 1,
                token: [0; 16],
                sender: UserId::new(sender).unwrap(),
                recipient: value.recipient,
                client_sequence: value.client_sequence,
                accepted_at: 1,
                presentation: value.presentation,
                queued: false,
                text: value.text,
                styles: value.styles,
                conference: value.conference,
            };
            let output = gg6_messages::received(&lease).unwrap();
            assert_eq!(output.kind, 10);
            assert!(output.payload.len() <= frame::MAX_PAYLOAD);
            assert_eq!(&output.payload[16..16 + encoded.len()], &encoded[..]);
        }
        "image" => {
            let Ok((recipient, value)) = gg6_images::decode(body) else {
                return false;
            };
            user(recipient.get());
            assert!((1..=255 * 1024).contains(&value.image().size));
            let output = gg6_images::received(UserId::new(1).unwrap(), 1, &value).unwrap();
            assert!((26..=1926).contains(&output.payload.len()));
            assert_eq!(&output.payload[18..26], &body[14..22]);
        }
        _ => panic!("unknown test target"),
    }
    true
}

fn checked(target: &str, body: &[u8], identity: &str) -> bool {
    match std::panic::catch_unwind(|| inspect(target, body)) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("REPRODUCE target={target} case={identity} bytes={body:02x?}");
            std::panic::resume_unwind(error)
        }
    }
}

#[test]
fn golden_corpus_all_prefixes_and_every_single_bit_mutation() {
    let mut prefixes = 0;
    let mut flips = 0;
    let mut names = std::collections::BTreeSet::new();
    for (name, target, body, accepted) in corpus() {
        assert!(names.insert(name.clone()));
        assert_eq!(
            checked(&target, &body, &name),
            accepted,
            "golden case {name}"
        );
        // A shorter prefix can legitimately be another valid request (e.g. empty PUT).
        for end in 0..=body.len() {
            checked(&target, &body[..end], &format!("{name}/prefix/{end}"));
            prefixes += 1;
        }
        for at in 0..body.len() {
            for bit in 0..8 {
                let mut changed = body.clone();
                changed[at] ^= 1 << bit;
                checked(&target, &changed, &format!("{name}/bit/{at}/{bit}"));
                flips += 1;
            }
        }
    }
    println!(
        "PROTOCOL_CORPUS_REPORT={}",
        json!({"cases": names.len(), "prefixes":prefixes,"single_bit_mutations":flips})
    );
}

struct Random(u64);
impl Random {
    // Test-only deterministic generator. Never used for keys, nonces or authentication.
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn index(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
    fn mutate(&mut self, bytes: &mut Vec<u8>) {
        if bytes.is_empty() {
            bytes.push(self.next() as u8);
            return;
        }
        match self.index(8) {
            0 => {
                let at = self.index(bytes.len());
                bytes[at] ^= 1 << self.index(8);
            }
            1 => bytes.truncate(self.index(bytes.len() + 1)),
            2 => {
                let at = self.index(bytes.len() + 1);
                bytes.insert(at, self.next() as u8);
            }
            3 => {
                let at = self.index(bytes.len());
                bytes.remove(at);
            }
            4 => {
                let at = self.index(bytes.len());
                bytes[at] = self.next() as u8;
            }
            5 => {
                for _ in 0..=self.index(16) {
                    bytes.push(self.next() as u8);
                }
            }
            6 if bytes.len() >= 4 => {
                let values = [0, 1, 31, 32, 70, 255, 1024, 2047, 4096, 65536, u32::MAX];
                let at = self.index(bytes.len() - 3);
                bytes[at..at + 4].copy_from_slice(&values[self.index(values.len())].to_le_bytes());
            }
            _ => {
                let at = self.index(bytes.len());
                let end = (at + 1 + self.index(32)).min(bytes.len());
                let copy = bytes[at..end].to_vec();
                bytes.splice(at..at, copy);
            }
        }
        bytes.truncate(INPUT_LIMIT);
    }
}
fn setting(name: &str, default: u64, maximum: u64) -> u64 {
    let value =
        std::env::var(name).map_or(default, |text| text.parse().expect("integer test setting"));
    assert!(
        (1..=maximum).contains(&value),
        "bounded test setting {name}"
    );
    value
}
#[test]
fn deterministic_mutation_campaign() {
    let seed = setting("RGG_PROTOCOL_SEED", 0x726767_20261102, u64::MAX);
    let iterations = setting("RGG_PROTOCOL_CASES", 20000, 2_000_000);
    let mut random = Random(seed);
    let cases = corpus();
    let mut accepted = [0_u64; 7];
    let mut rejected = [0_u64; 7];
    for index in 0..iterations {
        let (_, target, original, _) = &cases[random.index(cases.len())];
        let mut body = if index.is_multiple_of(16) {
            (0..random.index(4098))
                .map(|_| random.next() as u8)
                .collect()
        } else {
            original.clone()
        };
        for _ in 0..1 + random.index(4) {
            random.mutate(&mut body);
        }
        let result = checked(target, &body, &format!("seed/{seed}/iteration/{index}"));
        let at = TARGETS.iter().position(|name| name == target).unwrap();
        if result {
            accepted[at] += 1;
        } else {
            rejected[at] += 1;
        }
    }
    assert_eq!(
        accepted.iter().sum::<u64>() + rejected.iter().sum::<u64>(),
        iterations
    );
    println!(
        "PROTOCOL_MUTATION_REPORT={}",
        json!({"seed":seed,"iterations":iterations,
        "targets":TARGETS,"accepted":accepted,"rejected":rejected,"input_limit":INPUT_LIMIT,
        "coverage_guided":false})
    );
}

fn inspect_frames(raw: &[u8]) {
    let mut remaining = BytesMut::from(raw);
    loop {
        let before = remaining.clone();
        match frame::decode(&mut remaining) {
            Ok(Some(value)) => {
                let consumed = 8 + value.payload.len();
                assert_eq!(&frame::encode(&value).unwrap()[..], &before[..consumed]);
                assert_eq!(&remaining[..], &before[consumed..]);
                assert!(remaining.len() < before.len());
            }
            Ok(None) | Err(frame::FrameError::Oversized) => {
                assert_eq!(remaining, before);
                break;
            }
            Err(error) => panic!("unexpected streaming error: {error}"),
        }
    }
    assert_eq!(frame::finish(&remaining).is_ok(), remaining.is_empty());
}

#[test]
fn independent_frames_fragmentation_coalescing_and_length_boundaries() {
    for (_, _, body, _) in corpus() {
        // Construct framing independently, not via the serializer under test.
        let wire = [
            0x0bu32.to_le_bytes().as_slice(),
            &(body.len() as u32).to_le_bytes(),
            &body,
        ]
        .concat();
        for end in 0..wire.len() {
            let mut first = BytesMut::from(&wire[..end]);
            assert!(frame::decode(&mut first).unwrap().is_none());
            assert_eq!(&first[..], &wire[..end]);
            first.extend_from_slice(&wire[end..]);
            assert_eq!(
                frame::decode(&mut first).unwrap().unwrap().payload.as_ref(),
                body
            );
            assert!(first.is_empty());
        }
        let joined = [wire.as_slice(), &[8, 0, 0, 0, 0, 0, 0, 0], wire.as_slice()].concat();
        inspect_frames(&joined);
        for at in 0..8 {
            for byte in [0, 1, 127, 128, 255] {
                let mut changed = joined.clone();
                changed[at] = byte;
                inspect_frames(&changed);
            }
        }
    }
    for length in [0_u32, 1, 65535, 65536, 65537, u32::MAX] {
        let mut wire = [9u32.to_le_bytes(), length.to_le_bytes()].concat();
        if length as usize > frame::MAX_PAYLOAD {
            assert!(matches!(
                frame::decode(&mut BytesMut::from(wire.as_slice())),
                Err(frame::FrameError::Oversized)
            ));
        } else {
            wire.resize(8 + length as usize, 0xaf);
            inspect_frames(&wire);
            for end in [0, 1, 7, 8, wire.len() - 1] {
                inspect_frames(&wire[..end]);
            }
        }
    }
}
