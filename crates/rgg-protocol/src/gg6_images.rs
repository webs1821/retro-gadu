// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Independent wire implementation: libgadu-wire-reference / libgadu-legacy-reference.
use crate::{frame::Frame, gg6::DecodeError};
use bytes::{BufMut, BytesMut};
use rgg_application::images::{ImageControl, ImageId};
use rgg_domain::UserId;
pub fn decode(body: &[u8]) -> Result<(UserId, ImageControl), DecodeError> {
    if !(22..=1922).contains(&body.len()) || body[12] != 0 {
        return Err(DecodeError::Length);
    }
    let word = |p: usize| u32::from_le_bytes(body[p..p + 4].try_into().expect("checked"));
    let target = word(0);
    if target > 0xffffff || word(8) & !32 != 4 {
        return Err(DecodeError::Length);
    }
    let image = ImageId {
        size: word(14),
        crc32: word(18),
    };
    let tail = &body[22..];
    let control = match body[13] {
        4 if tail.is_empty() => ImageControl::Request(image),
        5 | 6 if tail.is_empty() => ImageControl::Missing(image),
        5 => {
            let end = tail
                .iter()
                .position(|b| *b == 0)
                .ok_or(DecodeError::Length)?;
            ImageControl::Start {
                image,
                filename: tail[..end].to_vec(),
                data: tail[end + 1..].to_vec(),
            }
        }
        6 => ImageControl::Continue {
            image,
            data: tail.to_vec(),
        },
        _ => return Err(DecodeError::Length),
    };
    control.validate().map_err(|_| DecodeError::Length)?;
    Ok((
        UserId::new(target).map_err(|_| DecodeError::UserNumber)?,
        control,
    ))
}
pub fn received(sender: UserId, time: u32, control: &ImageControl) -> Result<Frame, DecodeError> {
    if sender.get() > 0xffffff {
        return Err(DecodeError::UserNumber);
    }
    control.validate().map_err(|_| DecodeError::Length)?;
    let mut b = BytesMut::new();
    for value in [sender.get(), 0, time, 4] {
        b.put_u32_le(value);
    }
    b.put_u8(0);
    b.put_u8(match control {
        ImageControl::Request(_) => 4,
        ImageControl::Missing(_) | ImageControl::Start { .. } => 5,
        ImageControl::Continue { .. } => 6,
    });
    b.put_u32_le(control.image().size);
    b.put_u32_le(control.image().crc32);
    match control {
        ImageControl::Start { filename, data, .. } => {
            b.extend_from_slice(filename);
            b.put_u8(0);
            b.extend_from_slice(data);
        }
        ImageControl::Continue { data, .. } => b.extend_from_slice(data),
        _ => {}
    }
    Ok(Frame {
        kind: 10,
        payload: b.freeze(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packet(option: &[u8]) -> Vec<u8> {
        [vec![2, 0, 0, 0, 7, 0, 0, 0, 4, 0, 0, 0, 0], option.to_vec()].concat()
    }
    #[test]
    fn control_boundaries_and_missing_have_no_filename() {
        let id = ImageId {
            size: 3,
            crc32: 123,
        };
        for control in [
            ImageControl::Request(id),
            ImageControl::Missing(id),
            ImageControl::Start {
                image: id,
                filename: b"a.png".to_vec(),
                data: vec![1],
            },
            ImageControl::Continue {
                image: id,
                data: vec![2, 3],
            },
        ] {
            let wire = received(UserId::new(2).unwrap(), 100, &control).unwrap();
            let body = packet(&wire.payload[17..]);
            assert_eq!(decode(&body).unwrap().1, control);
            for end in 0..22 {
                assert!(decode(&body[..end.min(body.len())]).is_err());
            }
        }
        for option in [
            vec![4, 3, 0, 0, 0, 123, 0, 0, 0, 99],
            vec![5, 3, 0, 0, 0, 123, 0, 0, 0, b'a'],
            vec![5, 3, 0, 0, 0, 123, 0, 0, 0, b'/', 0],
            vec![4, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![4, 0, 0, 4, 0, 0, 0, 0, 0],
        ] {
            assert!(decode(&packet(&option)).is_err());
        }
        let wire = received(UserId::new(2).unwrap(), 0, &ImageControl::Missing(id)).unwrap();
        assert_eq!(wire.payload.len(), 26);
    }
}
