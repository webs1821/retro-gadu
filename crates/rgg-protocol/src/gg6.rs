// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Wire-layout references and compatibility observations: see THIRD_PARTY.toml.

use crate::frame::Frame;
use bytes::Bytes;
use encoding_rs::WINDOWS_1250;
use rgg_application::contracts::{Command, Event};
use rgg_domain::UserId;
use thiserror::Error;

pub const WELCOME: u32 = 0x0001;
pub const LOGIN60: u32 = 0x0015;

pub fn decode_control(frame: &Frame) -> Option<Command> {
    (frame.kind == 0x0008 && frame.payload.is_empty()).then_some(Command::Ping)
}

pub fn encode_control(event: Event) -> Frame {
    match event {
        Event::Pong => Frame {
            kind: 0x0007,
            payload: Bytes::new(),
        },
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("invalid GG6 login packet length")]
    Length,
    #[error("user number is outside the GG6 range")]
    UserNumber,
    #[error("description is not a valid bounded GG6 string")]
    Description,
    #[error("text cannot be represented in Windows-1250")]
    Encoding,
}

// No Debug derive: the structure contains a credential response.
pub struct Login60 {
    pub user: UserId,
    pub response: u32,
    pub status: u32,
    pub client_version: u32,
    pub image_limit_kib: u8,
    pub description: Option<String>,
    pub return_at: Option<u32>,
}

pub fn welcome(seed: u32) -> Frame {
    Frame {
        kind: WELCOME,
        payload: Bytes::copy_from_slice(&seed.to_le_bytes()),
    }
}

fn word(bytes: &[u8], start: usize) -> u32 {
    u32::from_le_bytes(
        bytes[start..start + 4]
            .try_into()
            .expect("validated length"),
    )
}

/// Legacy clients may delimit text with the packet boundary instead of NUL.
/// A timestamp is recognized only after an explicit NUL, never guessed from text.
/// Wire observation: external-libgadu170-validation in THIRD_PARTY.toml.
pub(crate) fn decode_description(
    tail: &[u8],
) -> Result<(Option<String>, Option<u32>), DecodeError> {
    if tail.is_empty() {
        return Ok((None, None));
    }
    let (end, timestamp) = match tail.iter().position(|byte| *byte == 0) {
        Some(end) => {
            let timestamp = match tail.len() - end - 1 {
                0 => None,
                4 => Some(word(tail, end + 1)),
                _ => return Err(DecodeError::Description),
            };
            (end, timestamp)
        }
        None => (tail.len(), None),
    };
    if end > 70 {
        return Err(DecodeError::Description);
    }
    let (text, invalid) = WINDOWS_1250.decode_without_bom_handling(&tail[..end]);
    if invalid {
        return Err(DecodeError::Encoding);
    }
    Ok((Some(text.into_owned()), timestamp))
}

pub fn decode_login(body: &[u8]) -> Result<Login60, DecodeError> {
    if !(31..=106).contains(&body.len()) {
        return Err(DecodeError::Length);
    }
    let number = word(body, 0);
    if number == 0 || number > 0x00ff_ffff {
        return Err(DecodeError::UserNumber);
    }
    let (description, return_at) = decode_description(&body[31..])?;
    Ok(Login60 {
        user: UserId::new(number).map_err(|_| DecodeError::UserNumber)?,
        response: word(body, 4),
        status: word(body, 8),
        client_version: word(body, 12),
        image_limit_kib: body[29],
        description,
        return_at,
    })
}

pub fn encode_text(text: &str) -> Result<Bytes, DecodeError> {
    if text.contains('\0') {
        return Err(DecodeError::Encoding);
    }
    let (encoded, _, invalid) = WINDOWS_1250.encode(text);
    if invalid {
        Err(DecodeError::Encoding)
    } else {
        Ok(Bytes::copy_from_slice(&encoded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login() -> Vec<u8> {
        let mut result = vec![0; 31];
        result[..4].copy_from_slice(&742858u32.to_le_bytes());
        result[8..12].copy_from_slice(&2u32.to_le_bytes());
        result[12..16].copy_from_slice(&0x20u32.to_le_bytes());
        result[29] = 255;
        result[30] = 0xbe;
        result
    }

    #[test]
    fn login_length_and_wire_limits_are_separate_from_domain_types() {
        let valid = login();
        assert_eq!(decode_login(&valid).unwrap().user.get(), 742858);
        for end in 0..31 {
            assert!(decode_login(&valid[..end]).is_err());
        }
        for number in [0u32, 0x01000000, u32::MAX] {
            let mut body = login();
            body[..4].copy_from_slice(&number.to_le_bytes());
            assert!(matches!(decode_login(&body), Err(DecodeError::UserNumber)));
        }
    }

    #[test]
    fn polish_description_timestamp_and_invalid_tails() {
        let mut body = login();
        body.extend_from_slice(&[0xaf, 0xf3, 0xb3, 0xe6, 0]); // Żółć, CP1250
        body.extend_from_slice(&1_700_000_000u32.to_le_bytes());
        let parsed = decode_login(&body).unwrap();
        assert_eq!(parsed.description.as_deref(), Some("Żółć"));
        assert_eq!(parsed.return_at, Some(1_700_000_000));
        body.pop();
        assert!(decode_login(&body).is_err());
        let mut unterminated = login();
        unterminated.extend_from_slice(b"text");
        assert_eq!(
            decode_login(&unterminated).unwrap().description.as_deref(),
            Some("text")
        );
    }

    #[test]
    fn packet_boundary_descriptions_keep_limits_and_explicit_time_delimiters() {
        for length in [1, 69, 70, 71, 75] {
            let text = vec![b'x'; length];
            for terminated in [false, true] {
                let mut tail = text.clone();
                if terminated {
                    tail.push(0);
                }
                let mut body = login();
                body.extend_from_slice(&tail);
                assert_eq!(decode_login(&body).is_ok(), length <= 70);
                assert_eq!(decode_description(&tail).is_ok(), length <= 70);
            }
        }
        for length in [1, 2, 3, 5] {
            let mut tail = b"text\0".to_vec();
            tail.extend(vec![1; length]);
            assert!(decode_description(&tail).is_err());
        }
        let (text, time) = decode_description(b"textabcd").unwrap();
        assert_eq!(text.as_deref(), Some("textabcd"));
        assert_eq!(time, None);
        let (text, time) = decode_description(&[0xaf, 0xf3, 0xb3, 0xe6]).unwrap();
        assert_eq!(text.as_deref(), Some("Żółć"));
        assert_eq!(time, None);
        assert_eq!(decode_description(b""), Ok((None, None)));
        assert_eq!(
            decode_description(b"\0\0\0\0\0"),
            Ok((Some(String::new()), Some(0)))
        );
    }

    #[test]
    fn encoding_never_silently_replaces_unsupported_characters() {
        assert_eq!(&encode_text("Żółć").unwrap()[..], &[0xaf, 0xf3, 0xb3, 0xe6]);
        assert!(encode_text("🙂").is_err());
        assert!(encode_text("a\0b").is_err());
    }
}
