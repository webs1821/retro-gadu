// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Wire facts: libgadu-wire-reference / libgadu-legacy-reference in THIRD_PARTY.toml.
use crate::{
    frame::Frame,
    gg6::{DecodeError, Login60, encode_text},
    gg6_messages,
};
use bytes::{BufMut, BytesMut};
use encoding_rs::WINDOWS_1250;
use rgg_application::presence::{Availability, Presence, VisiblePresence};
fn make(
    value: u32,
    description: Option<String>,
    return_at: Option<u32>,
) -> Result<Presence, DecodeError> {
    gg6_messages::status(value)?;
    let availability = match value & 0xff {
        1 | 0x15 => Availability::Offline,
        2 | 4 => Availability::Available,
        3 | 5 => Availability::Away,
        _ => Availability::Invisible,
    };
    let description = description.filter(|s| !s.is_empty() || return_at.is_some());
    let p = Presence {
        availability,
        description,
        return_at,
        friends_only: value & 0x8000 != 0,
    };
    p.validate().map_err(|_| DecodeError::Description)?;
    Ok(p)
}
pub fn from_login(login: &Login60) -> Result<Presence, DecodeError> {
    make(login.status, login.description.clone(), login.return_at)
}
pub fn decode_status(body: &[u8]) -> Result<Presence, DecodeError> {
    gg6_messages::decode_status(body)?;
    let status = u32::from_le_bytes(body[..4].try_into().expect("validated"));
    if body.len() == 4 {
        return make(status, None, None);
    }
    let tail = &body[4..];
    let end = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or(DecodeError::Description)?;
    let (text, bad) = WINDOWS_1250.decode_without_bom_handling(&tail[..end]);
    if bad {
        return Err(DecodeError::Encoding);
    }
    let time = if tail.len() == end + 5 {
        Some(u32::from_le_bytes(
            tail[end + 1..].try_into().expect("validated"),
        ))
    } else {
        None
    };
    make(status, Some(text.into_owned()), time)
}
fn entry(out: &mut BytesMut, view: &VisiblePresence, sized: bool) -> Result<(), DecodeError> {
    let p = &view.presence;
    p.validate().map_err(|_| DecodeError::Description)?;
    let described = p.description.is_some();
    let status = match (p.availability, described) {
        (Availability::Offline, false) => 1,
        (Availability::Offline, true) => 0x15,
        (Availability::Available, false) => 2,
        (Availability::Available, true) => 4,
        (Availability::Away, false) => 3,
        (Availability::Away, true) => 5,
        (Availability::Invisible, _) => return Err(DecodeError::Description),
    };
    out.put_u32_le(view.user.get());
    out.put_u8(status);
    // DCC addresses, client fingerprints and image capability are not published in this profile.
    out.extend_from_slice(&[0; 9]);
    if let Some(text) = &p.description {
        let bytes = encode_text(text)?;
        if bytes.len() > 70 {
            return Err(DecodeError::Description);
        }
        let mut tail = BytesMut::new();
        tail.extend_from_slice(&bytes);
        tail.put_u8(0);
        if let Some(time) = p.return_at {
            tail.put_u32_le(time);
        }
        if sized {
            out.put_u8(tail.len() as u8);
        }
        out.extend_from_slice(&tail);
    }
    Ok(())
}
pub fn snapshot(views: &[VisiblePresence]) -> Result<Frame, DecodeError> {
    if views.len() > 400 {
        return Err(DecodeError::Length);
    }
    let mut bytes = BytesMut::new();
    for view in views {
        entry(&mut bytes, view, true)?;
    }
    Ok(Frame {
        kind: 0x11,
        payload: bytes.freeze(),
    })
}
pub fn changed(view: &VisiblePresence) -> Result<Frame, DecodeError> {
    let mut bytes = BytesMut::new();
    entry(&mut bytes, view, false)?;
    Ok(Frame {
        kind: 0xf,
        payload: bytes.freeze(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use rgg_domain::UserId;
    #[test]
    fn description_boundaries_and_distinct_snapshot_change_layouts() {
        let mut packet = vec![0x15, 0x80, 0, 0];
        packet.extend_from_slice(&[0xaf, 0xf3, 0xb3, 0xe6, 0]);
        packet.extend_from_slice(&123u32.to_le_bytes());
        let p = decode_status(&packet).unwrap();
        assert!(p.friends_only);
        assert_eq!(p.description.as_deref(), Some("Żółć"));
        assert_eq!(p.return_at, Some(123));
        let v = VisiblePresence {
            user: UserId::new(5).unwrap(),
            presence: p,
        };
        let a = snapshot(std::slice::from_ref(&v)).unwrap();
        let b = changed(&v).unwrap();
        assert_eq!(a.payload[14], 9);
        assert_eq!(&a.payload[15..], &packet[4..]);
        assert_eq!(&b.payload[14..], &packet[4..]);
        for n in [0, 1, 69, 70, 71] {
            let mut body = vec![4, 0, 0, 0];
            body.extend(vec![b'x'; n]);
            body.push(0);
            assert_eq!(decode_status(&body).is_ok(), n <= 70);
        }
        let mut v = v;
        v.presence.availability = Availability::Invisible;
        assert!(changed(&v).is_err());
    }
}
