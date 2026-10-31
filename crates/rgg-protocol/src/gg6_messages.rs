// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Independent implementation of libgadu-wire-reference; see THIRD_PARTY.toml.
use crate::{
    frame::Frame,
    gg6::{DecodeError, encode_text},
};
use bytes::{BufMut, Bytes, BytesMut};
use encoding_rs::WINDOWS_1250;
use rgg_application::messaging::{ContactRule, DeliveryLease, MessageBundle, TextStyle};
use rgg_domain::{MessageText, UserId};

pub fn empty(kind: u32) -> Frame {
    Frame {
        kind,
        payload: Bytes::new(),
    }
}
fn word(body: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(body[at..at + 4].try_into().expect("length checked"))
}
pub fn inbound_limit(kind: u32) -> usize {
    match kind {
        0x15 => 106,
        0x16 => 2048,
        0x14 => 4096,
        0x8 | 0x12 => 0,
        0xf | 0x10 => 2000,
        0xd | 0xe => 5,
        0xb => 4096,
        0x2 => 79,
        _ => 0,
    }
}
pub fn status(value: u32) -> Result<(bool, bool), DecodeError> {
    if value & !0x80ff != 0 {
        return Err(DecodeError::Length);
    }
    let base = value & 0xff;
    if ![1, 2, 3, 4, 5, 0x14, 0x15, 0x16].contains(&base) {
        return Err(DecodeError::Length);
    }
    Ok(([1, 0x15].contains(&base), value & 0x8000 != 0))
}
pub fn decode_status(body: &[u8]) -> Result<(bool, bool), DecodeError> {
    if !(4..=79).contains(&body.len()) {
        return Err(DecodeError::Length);
    }
    let value = word(body, 0);
    if body.len() > 4 {
        let end = body[4..]
            .iter()
            .position(|b| *b == 0)
            .ok_or(DecodeError::Description)?;
        if end > 70 || ![0, 4].contains(&(body.len() - 5 - end)) {
            return Err(DecodeError::Description);
        }
        if WINDOWS_1250
            .decode_without_bom_handling(&body[4..4 + end])
            .1
        {
            return Err(DecodeError::Encoding);
        }
    }
    status(value)
}
pub fn contacts(body: &[u8]) -> Result<Vec<ContactRule>, DecodeError> {
    if body.len() > 2000 || !body.len().is_multiple_of(5) {
        return Err(DecodeError::Length);
    }
    body.as_chunks::<5>()
        .0
        .iter()
        .map(|b| {
            let number = word(b, 0);
            if number > 0xffffff || b[4] & !7 != 0 {
                return Err(DecodeError::UserNumber);
            }
            Ok(ContactRule {
                user: UserId::new(number).map_err(|_| DecodeError::UserNumber)?,
                flags: b[4],
            })
        })
        .collect()
}
pub fn message_header(body: &[u8]) -> Option<(u32, u32, bool)> {
    (body.len() >= 12).then(|| (word(body, 0), word(body, 4), word(body, 8) & 32 != 0))
}
pub fn decode_message(body: &[u8]) -> Result<MessageBundle, DecodeError> {
    if !(13..=4096).contains(&body.len()) {
        return Err(DecodeError::Length);
    }
    let target = word(body, 0);
    let class = word(body, 8);
    if target > 0xffffff || class & !(1 | 4 | 8 | 32) != 0 {
        return Err(DecodeError::Length);
    }
    let end = body[12..]
        .iter()
        .position(|b| *b == 0)
        .ok_or(DecodeError::Length)?;
    if end == 0 || end > 2000 {
        return Err(DecodeError::Length);
    }
    let (text, bad) = WINDOWS_1250.decode_without_bom_handling(&body[12..12 + end]);
    if bad {
        return Err(DecodeError::Encoding);
    }
    let mut options = &body[13 + end..];
    let mut styles = Vec::new();
    if !options.is_empty() {
        if options.len() < 3 || options[0] != 2 {
            return Err(DecodeError::Length);
        }
        let len = usize::from(u16::from_le_bytes([options[1], options[2]]));
        if options.len() != len + 3 {
            return Err(DecodeError::Length);
        }
        options = &options[3..];
        while !options.is_empty() {
            if options.len() < 3 || styles.len() >= 256 {
                return Err(DecodeError::Length);
            }
            let position = u16::from_le_bytes([options[0], options[1]]);
            let flags = options[2];
            options = &options[3..];
            if flags & !15 != 0 || usize::from(position) > end {
                return Err(DecodeError::Length);
            }
            let color = if flags & 8 != 0 {
                if options.len() < 3 {
                    return Err(DecodeError::Length);
                }
                let rgb = [options[0], options[1], options[2]];
                options = &options[3..];
                Some(rgb)
            } else {
                None
            };
            styles.push(TextStyle {
                position,
                bold: flags & 1 != 0,
                italic: flags & 2 != 0,
                underline: flags & 4 != 0,
                color,
            });
        }
    }
    Ok(MessageBundle {
        recipient: UserId::new(target).map_err(|_| DecodeError::UserNumber)?,
        client_sequence: word(body, 4),
        presentation: (class & 12) as u8,
        text: MessageText::new(text.into_owned()).map_err(|_| DecodeError::Length)?,
        styles,
    })
}
pub fn acknowledgement(status: u32, recipient: u32, sequence: u32) -> Frame {
    let mut b = BytesMut::with_capacity(12);
    for value in [status, recipient, sequence] {
        b.put_u32_le(value);
    }
    Frame {
        kind: 5,
        payload: b.freeze(),
    }
}
pub fn received(lease: &DeliveryLease) -> Result<Frame, DecodeError> {
    let text = encode_text(lease.text.as_str())?;
    if text.len() > 2000 || lease.styles.len() > 256 {
        return Err(DecodeError::Length);
    }
    let mut b = BytesMut::new();
    for v in [
        lease.sender.get(),
        lease.client_sequence,
        u32::try_from(lease.accepted_at).map_err(|_| DecodeError::Length)?,
        u32::from(lease.presentation) | u32::from(lease.queued),
    ] {
        b.put_u32_le(v);
    }
    b.extend_from_slice(&text);
    b.put_u8(0);
    if !lease.styles.is_empty() {
        let mut styles = BytesMut::new();
        for s in &lease.styles {
            if usize::from(s.position) > text.len() {
                return Err(DecodeError::Length);
            }
            styles.put_u16_le(s.position);
            styles.put_u8(
                u8::from(s.bold)
                    | (u8::from(s.italic) * 2)
                    | (u8::from(s.underline) * 4)
                    | (u8::from(s.color.is_some()) * 8),
            );
            if let Some(rgb) = s.color {
                styles.extend_from_slice(&rgb);
            }
        }
        b.put_u8(2);
        b.put_u16_le(styles.len() as u16);
        b.extend_from_slice(&styles);
    }
    Ok(Frame {
        kind: 0xa,
        payload: b.freeze(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_roundtrip_strips_untrusted_flags_and_preserves_polish_styles() {
        let mut b = BytesMut::new();
        for v in [42u32, 7, 1 | 8 | 32] {
            b.put_u32_le(v);
        }
        b.extend_from_slice(&[0xaf, 0xf3, 0xb3, 0xe6, 0, 2, 6, 0, 0, 0, 9, 255, 0, 127]);
        let m = decode_message(&b).unwrap();
        assert_eq!(m.text.as_str(), "Żółć");
        assert_eq!(m.presentation, 8);
        assert_eq!(m.styles[0].color, Some([255, 0, 127]));
        let l = DeliveryLease {
            id: 1,
            token: [0; 16],
            sender: UserId::new(1).unwrap(),
            client_sequence: 7,
            accepted_at: 42,
            presentation: m.presentation,
            queued: false,
            text: m.text,
            styles: m.styles,
        };
        let f = received(&l).unwrap();
        assert_eq!(word(&f.payload, 12), 8);
        assert_eq!(&f.payload[16..], &b[12..]);
        for end in 0..13 {
            assert!(decode_message(&b[..end]).is_err());
        }
        b[8] = 16;
        assert!(decode_message(&b).is_err());
        b[8] = 8;
        b[22] = 128;
        assert!(decode_message(&b).is_err());
    }
    #[test]
    fn contacts_and_status_bounds() {
        assert!(contacts(&[1, 0, 0, 0, 8]).is_err());
        assert!(contacts(&[1, 0, 0, 0]).is_err());
        assert_eq!(status(0x8014).unwrap(), (false, true));
        assert!(status(6).is_err());
        assert!(status(0x10002).is_err());
        assert!(decode_status(&[2, 0, 0, 0, b'x']).is_err());
    }
}
