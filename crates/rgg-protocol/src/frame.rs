// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Wire format reference: https://libgadu.github.io/protocol/#ch1.1

use bytes::{Buf, Bytes, BytesMut};
use thiserror::Error;

pub const MAX_PAYLOAD: usize = 64 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("packet payload exceeds the configured protocol limit")]
    Oversized,
    #[error("stream ended inside a packet")]
    Truncated,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: u32,
    pub payload: Bytes,
}

/// Does not reserve memory from an untrusted length. The transport must also bound reads.
pub fn decode(buffer: &mut BytesMut) -> Result<Option<Frame>, FrameError> {
    if buffer.len() < 8 {
        return Ok(None);
    }
    let kind = u32::from_le_bytes(buffer[..4].try_into().expect("four bytes"));
    let length = u32::from_le_bytes(buffer[4..8].try_into().expect("four bytes")) as usize;
    if length > MAX_PAYLOAD {
        return Err(FrameError::Oversized);
    }
    if buffer.len() < 8 + length {
        return Ok(None);
    }
    buffer.advance(8);
    Ok(Some(Frame {
        kind,
        payload: buffer.split_to(length).freeze(),
    }))
}

pub fn encode(frame: &Frame) -> Result<Bytes, FrameError> {
    if frame.payload.len() > MAX_PAYLOAD {
        return Err(FrameError::Oversized);
    }
    let mut result = BytesMut::with_capacity(8 + frame.payload.len());
    result.extend_from_slice(&frame.kind.to_le_bytes());
    result.extend_from_slice(&(frame.payload.len() as u32).to_le_bytes());
    result.extend_from_slice(&frame.payload);
    Ok(result.freeze())
}

/// Call after draining complete frames when the transport reaches EOF.
pub fn finish(buffer: &BytesMut) -> Result<(), FrameError> {
    if buffer.is_empty() {
        Ok(())
    } else {
        Err(FrameError::Truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WELCOME: &[u8] = &[1, 0, 0, 0, 4, 0, 0, 0, 0x78, 0x56, 0x34, 0x12];

    #[test]
    fn fragmented_header_and_payload_preserve_every_byte() {
        for split in 0..WELCOME.len() {
            let mut buffer = BytesMut::from(&WELCOME[..split]);
            assert!(decode(&mut buffer).unwrap().is_none());
            assert_eq!(&buffer[..], &WELCOME[..split]);
            buffer.extend_from_slice(&WELCOME[split..]);
            let frame = decode(&mut buffer).unwrap().unwrap();
            assert_eq!(frame.kind, 1);
            assert_eq!(&frame.payload[..], &[0x78, 0x56, 0x34, 0x12]);
            assert_eq!(&encode(&frame).unwrap()[..], WELCOME);
            assert!(buffer.is_empty());
        }
    }

    #[test]
    fn coalesced_packets_and_unknown_kinds_keep_their_boundaries() {
        let mut buffer = BytesMut::from(WELCOME);
        buffer.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0]);
        assert_eq!(decode(&mut buffer).unwrap().unwrap().kind, 1);
        assert_eq!(decode(&mut buffer).unwrap().unwrap().kind, u32::MAX);
        assert!(decode(&mut buffer).unwrap().is_none());
        assert_eq!(finish(&buffer), Ok(()));
    }

    #[test]
    fn rejects_large_lengths_before_waiting_for_the_body_and_detects_eof() {
        let mut buffer = BytesMut::from(&[1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff][..]);
        assert!(matches!(decode(&mut buffer), Err(FrameError::Oversized)));
        assert_eq!(buffer.len(), 8);
        assert_eq!(
            finish(&BytesMut::from(&WELCOME[..9])),
            Err(FrameError::Truncated)
        );
        assert!(matches!(
            encode(&Frame {
                kind: 1,
                payload: Bytes::from(vec![0; MAX_PAYLOAD + 1])
            }),
            Err(FrameError::Oversized)
        ));
    }
}
