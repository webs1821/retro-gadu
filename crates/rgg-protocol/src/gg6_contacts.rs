// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Restricted legacy completion contract: see docs/implementation/PRESENCE-AND-CONTACTS.md.
use crate::{
    frame::Frame,
    gg6::{DecodeError, encode_text},
};
use bytes::{BufMut, BytesMut};
use encoding_rs::WINDOWS_1250;
use rgg_application::contacts::ArchiveSegment;
pub enum Request {
    Get,
    Put(ArchiveSegment),
}
pub fn decode(body: &[u8]) -> Result<Request, DecodeError> {
    if body.is_empty() || body.len() > 2048 {
        return Err(DecodeError::Length);
    }
    match body[0] {
        2 if body.len() == 1 => Ok(Request::Get),
        0 | 1 => {
            let tail = &body[1..];
            if tail.contains(&0) {
                return Err(DecodeError::Encoding);
            }
            let (text, bad) = WINDOWS_1250.decode_without_bom_handling(tail);
            if bad {
                return Err(DecodeError::Encoding);
            }
            Ok(Request::Put(ArchiveSegment {
                start: body[0] == 0,
                complete: tail.len() < 2047,
                text: text.into_owned(),
            }))
        }
        _ => Err(DecodeError::Length),
    }
}
pub fn reply(kind: u8, data: &[u8]) -> Frame {
    let mut bytes = BytesMut::new();
    bytes.put_u8(kind);
    bytes.extend_from_slice(data);
    Frame {
        kind: 0x10,
        payload: bytes.freeze(),
    }
}
pub fn export(text: &str) -> Result<Vec<Frame>, DecodeError> {
    let bytes = encode_text(text)?;
    if bytes.len() > 262144 {
        return Err(DecodeError::Length);
    }
    if bytes.is_empty() {
        return Ok(vec![reply(6, &[])]);
    }
    let count = bytes.len().div_ceil(2047);
    Ok(bytes
        .chunks(2047)
        .enumerate()
        .map(|(i, part)| reply(if i + 1 == count { 6 } else { 4 }, part))
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_chunk_is_ambiguous_and_export_has_an_explicit_last_type() {
        for n in [0, 1, 2046, 2047, 2048, 4094, 4095] {
            let text = "a".repeat(n);
            let frames = export(&text).unwrap();
            assert_eq!(frames.last().unwrap().payload[0], 6);
            assert_eq!(
                frames
                    .iter()
                    .flat_map(|f| f.payload[1..].iter().copied())
                    .collect::<Vec<_>>(),
                text.as_bytes()
            );
        }
        for n in [0, 1, 2046, 2047] {
            let mut body = vec![0];
            body.extend(vec![b'a'; n]);
            let Request::Put(segment) = decode(&body).unwrap() else {
                panic!()
            };
            assert_eq!(segment.complete, n < 2047);
        }
        assert!(decode(&[2, 1]).is_err());
        assert!(decode(&[0, 0]).is_err());
    }
}
