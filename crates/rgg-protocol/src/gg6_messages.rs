// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Independent implementation of libgadu-wire-reference; see THIRD_PARTY.toml.
use crate::{
    frame::Frame,
    gg6::{DecodeError, decode_description, encode_text},
};
use bytes::{BufMut, Bytes, BytesMut};
use encoding_rs::WINDOWS_1250;
use rgg_application::conferences::{ConferenceContext, MAX_OTHER_PARTICIPANTS};
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
    decode_description(&body[4..])?;
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
    let mut conference = ConferenceContext::default();
    let mut seen_conference = false;
    let mut seen_styles = false;
    while !options.is_empty() {
        match options[0] {
            1 if !seen_conference => {
                seen_conference = true;
                if options.len() < 5 {
                    return Err(DecodeError::Length);
                }
                let count = word(options, 1) as usize;
                if count > MAX_OTHER_PARTICIPANTS || options.len() < 5 + 4 * count {
                    return Err(DecodeError::Length);
                }
                let mut others = Vec::with_capacity(count);
                for i in 0..count {
                    let value = word(options, 5 + 4 * i);
                    if value > 0xffffff || value == target {
                        return Err(DecodeError::UserNumber);
                    }
                    others.push(UserId::new(value).map_err(|_| DecodeError::UserNumber)?);
                }
                conference = ConferenceContext::new(others).map_err(|_| DecodeError::Length)?;
                options = &options[5 + 4 * count..];
            }
            2 if !seen_styles => {
                seen_styles = true;
                if options.len() < 3 {
                    return Err(DecodeError::Length);
                }
                let len = usize::from(u16::from_le_bytes([options[1], options[2]]));
                if options.len() < 3 + len {
                    return Err(DecodeError::Length);
                }
                let mut rich = &options[3..3 + len];
                options = &options[3 + len..];
                while !rich.is_empty() {
                    if rich.len() < 3 || styles.len() >= 256 {
                        return Err(DecodeError::Length);
                    }
                    let position = u16::from_le_bytes([rich[0], rich[1]]);
                    let flags = rich[2];
                    rich = &rich[3..];
                    if flags & !143 != 0 || usize::from(position) > end {
                        return Err(DecodeError::Length);
                    }
                    let color = if flags & 8 != 0 {
                        if rich.len() < 3 {
                            return Err(DecodeError::Length);
                        }
                        let rgb = [rich[0], rich[1], rich[2]];
                        rich = &rich[3..];
                        Some(rgb)
                    } else {
                        None
                    };
                    let image = if flags & 128 != 0 {
                        if rich.len() < 10 || rich[..2] != [9, 1] {
                            return Err(DecodeError::Length);
                        }
                        let id = rgg_application::images::ImageId {
                            size: word(rich, 2),
                            crc32: word(rich, 6),
                        };
                        id.validate().map_err(|_| DecodeError::Length)?;
                        rich = &rich[10..];
                        Some(id)
                    } else {
                        None
                    };
                    styles.push(TextStyle {
                        position,
                        bold: flags & 1 != 0,
                        italic: flags & 2 != 0,
                        underline: flags & 4 != 0,
                        color,
                        image,
                    });
                }
            }
            _ => return Err(DecodeError::Length),
        }
    }
    rgg_application::images::references(&styles).map_err(|_| DecodeError::Length)?;
    Ok(MessageBundle {
        recipient: UserId::new(target).map_err(|_| DecodeError::UserNumber)?,
        client_sequence: word(body, 4),
        presentation: (class & 12) as u8,
        text: MessageText::new(text.into_owned()).map_err(|_| DecodeError::Length)?,
        styles,
        conference,
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
    if lease.sender.get() > 0xffffff || lease.recipient.get() > 0xffffff {
        return Err(DecodeError::UserNumber);
    }
    lease
        .conference
        .validate_for(lease.sender, lease.recipient)
        .map_err(|_| DecodeError::Length)?;
    rgg_application::images::references(&lease.styles).map_err(|_| DecodeError::Length)?;
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
    if !lease.conference.others().is_empty() {
        b.put_u8(1);
        b.put_u32_le(lease.conference.others().len() as u32);
        for user in lease.conference.others() {
            if user.get() > 0xffffff {
                return Err(DecodeError::UserNumber);
            }
            b.put_u32_le(user.get());
        }
    }
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
                    | (u8::from(s.color.is_some()) * 8)
                    | (u8::from(s.image.is_some()) * 128),
            );
            if let Some(rgb) = s.color {
                styles.extend_from_slice(&rgb);
            }
            if let Some(image) = s.image {
                styles.extend_from_slice(&[9, 1]);
                styles.put_u32_le(image.size);
                styles.put_u32_le(image.crc32);
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
    fn image_descriptor_after_color_roundtrips_and_rejects_truncation_and_limits() {
        let mut body = BytesMut::new();
        for v in [2u32, 7, 8] {
            body.put_u32_le(v);
        }
        body.extend_from_slice(&[160, 0, 2, 16, 0, 0, 0, 136, 255, 0, 127, 9, 1]);
        body.put_u32_le(255 * 1024);
        body.put_u32_le(0x12345678);
        let m = decode_message(&body).unwrap();
        assert_eq!(m.styles[0].image.unwrap().size, 255 * 1024);
        let lease = DeliveryLease {
            id: 1,
            token: [0; 16],
            sender: UserId::new(1).unwrap(),
            recipient: m.recipient,
            client_sequence: 7,
            accepted_at: 100,
            presentation: m.presentation,
            queued: false,
            text: m.text,
            styles: m.styles,
            conference: m.conference,
        };
        assert_eq!(&received(&lease).unwrap().payload[16..], &body[12..]);
        for end in 15..body.len() {
            assert!(decode_message(&body[..end]).is_err());
        }
        for size in [0, 255 * 1024 + 1, u32::MAX] {
            let mut invalid = body.to_vec();
            invalid[25..29].copy_from_slice(&size.to_le_bytes());
            assert!(decode_message(&invalid).is_err());
        }
        let mut too_many = body[..14].to_vec();
        let mut rich = Vec::new();
        for crc in 0u32..9 {
            rich.extend_from_slice(&[0, 0, 128, 9, 1]);
            rich.extend_from_slice(&1u32.to_le_bytes());
            rich.extend_from_slice(&crc.to_le_bytes());
        }
        too_many.push(2);
        too_many.extend_from_slice(&(rich.len() as u16).to_le_bytes());
        too_many.extend(rich);
        assert!(decode_message(&too_many).is_err());
    }
    #[test]
    fn conference_options_preserve_order_accept_styles_in_either_order_and_bound_counts() {
        let header = [2u32, 9, 8]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .chain(b"text\0".iter().copied())
            .collect::<Vec<_>>();
        let group = [
            vec![1],
            2u32.to_le_bytes().to_vec(),
            4u32.to_le_bytes().to_vec(),
            3u32.to_le_bytes().to_vec(),
        ]
        .concat();
        let rich = vec![2, 3, 0, 0, 0, 1];
        for options in [
            [group.clone(), rich.clone()].concat(),
            [rich.clone(), group.clone()].concat(),
        ] {
            let m = decode_message(&[header.clone(), options].concat()).unwrap();
            assert_eq!(
                m.conference
                    .others()
                    .iter()
                    .map(|u| u.get())
                    .collect::<Vec<_>>(),
                [4, 3]
            );
            let lease = DeliveryLease {
                id: 1,
                token: [0; 16],
                sender: UserId::new(1).unwrap(),
                recipient: m.recipient,
                client_sequence: 9,
                accepted_at: 100,
                presentation: m.presentation,
                queued: true,
                text: m.text,
                styles: m.styles,
                conference: m.conference,
            };
            assert_eq!(
                &received(&lease).unwrap().payload[16..],
                &[b"text\0".to_vec(), group.clone(), rich.clone()].concat()
            );
        }
        let invalid = [
            vec![1],
            vec![1, 255, 255, 255, 255],
            vec![1, 31, 0, 0, 0],
            vec![1, 1, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 2, 0, 0, 0],
            vec![1, 1, 0, 0, 0, 0, 0, 0, 1],
            vec![1, 2, 0, 0, 0, 3, 0, 0, 0, 3, 0, 0, 0],
            [group.clone(), group].concat(),
            [rich.clone(), rich].concat(),
        ];
        for options in invalid {
            assert!(decode_message(&[header.clone(), options].concat()).is_err());
        }
        assert!(
            decode_message(&[header, vec![1, 0, 0, 0, 0]].concat())
                .unwrap()
                .conference
                .others()
                .is_empty()
        );
    }
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
            recipient: UserId::new(42).unwrap(),
            client_sequence: 7,
            accepted_at: 42,
            presentation: m.presentation,
            queued: false,
            text: m.text,
            styles: m.styles,
            conference: m.conference,
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
        assert!(decode_status(&[2, 0, 0, 0, b'x']).is_ok());
        assert!(decode_status(&[2, 0, 0, 0, b'x', 0, 1]).is_err());
    }
}
