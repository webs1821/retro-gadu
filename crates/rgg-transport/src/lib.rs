// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

//! Transport framing. Application commands are selected by a profile, never by SQL code.
use bytes::BytesMut;
use rgg_protocol::frame::{self, Frame, FrameError};
use tokio_util::codec::{Decoder, Encoder};
pub mod server;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("network I/O failed")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Frame(#[from] FrameError),
}

#[derive(Default)]
pub struct FrameCodec;

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = TransportError;
    fn decode(&mut self, input: &mut BytesMut) -> Result<Option<Frame>, Self::Error> {
        Ok(frame::decode(input)?)
    }
    fn decode_eof(&mut self, input: &mut BytesMut) -> Result<Option<Frame>, Self::Error> {
        let frame = self.decode(input)?;
        if frame.is_none() {
            frame::finish(input)?;
        }
        Ok(frame)
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = TransportError;
    fn encode(&mut self, frame: Frame, output: &mut BytesMut) -> Result<(), Self::Error> {
        output.extend_from_slice(&frame::encode(&frame)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    use tokio_util::codec::Framed;

    #[tokio::test]
    async fn real_tcp_fragmentation_crosses_protocol_application_and_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut framed = Framed::new(stream, FrameCodec);
            let incoming = framed.next().await.unwrap().unwrap();
            let command = rgg_protocol::gg6::decode_control(&incoming).unwrap();
            let event = rgg_application::handle_control(command);
            framed
                .send(rgg_protocol::gg6::encode_control(event))
                .await
                .unwrap();
            assert!(matches!(
                framed.next().await,
                Some(Err(TransportError::Frame(FrameError::Truncated)))
            ));
        });
        let mut client = TcpStream::connect(address).await.unwrap();
        for byte in [8u8, 0, 0, 0, 0, 0, 0, 0] {
            client.write_all(&[byte]).await.unwrap();
        }
        let mut pong = [0; 8];
        client.read_exact(&mut pong).await.unwrap();
        assert_eq!(pong, [7, 0, 0, 0, 0, 0, 0, 0]);
        client.write_all(&[1, 0]).await.unwrap();
        client.shutdown().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
    }
}
