//! The websocket transport, server side: the handshake answer and the
//! frame codec the protocol rides on. Text frames carry the JSON protocol;
//! ping, pong, and close are the only other frames a peer may send —
//! anything else is a protocol violation that ends the connection, because
//! this server speaks with browsers it serves itself and never negotiates
//! extensions.

use std::io;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The magic GUID the handshake answer hashes the client key with (RFC 6455).
const HANDSHAKE_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The largest message the server reads, so a peer cannot grow the server's
/// memory without bound. A larger frame or message ends the connection.
const MAX_MESSAGE: usize = 1024 * 1024;

/// A message read off the wire.
#[derive(Debug, PartialEq)]
pub enum Message {
    Text(String),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

/// The `Sec-WebSocket-Accept` answer for a client's handshake key. The
/// handshake needs SHA-1 and base64 once, for this one fixed string — two
/// dependency-light crates for that beat owning the primitives.
pub fn accept_key(client_key: &str) -> String {
    let digest = sha1_smol::Sha1::from(format!("{client_key}{HANDSHAKE_GUID}"))
        .digest()
        .bytes();
    STANDARD.encode(digest)
}

/// One connection's message stream: read messages off it one at a time.
///
/// The half-assembled fragmented message is state of the stream, not of one
/// read — a control frame answered while a message is in flight leaves the
/// fragment open for the continuation that follows it, and the messages come
/// out in wire order.
pub struct Reader {
    fragment: Option<Vec<u8>>,
}

impl Reader {
    /// A fresh message stream.
    pub fn new() -> Reader {
        Reader { fragment: None }
    }

    /// Read one message. Returns `None` when the peer closed the connection.
    ///
    /// A message that violates the framing rules — an unmasked frame, a
    /// reserved bit, a frame over the size cap, a bad opcode — is an error
    /// that ends the connection; the caller responds by dropping it.
    pub async fn read(
        &mut self,
        reader: &mut (impl AsyncRead + Unpin),
    ) -> io::Result<Option<Message>> {
        loop {
            let (fin, opcode, payload) = read_frame(reader).await?;
            match opcode {
                0x0 => {
                    let accumulated = self
                        .fragment
                        .as_mut()
                        .ok_or_else(|| invalid("a continuation frame with no message open"))?;
                    accumulated.extend_from_slice(&payload);
                    if accumulated.len() > MAX_MESSAGE {
                        return Err(too_large());
                    }
                    if fin {
                        let text = utf8(
                            self.fragment
                                .take()
                                .expect("a continuation implies a message open"),
                        )?;
                        return Ok(Some(Message::Text(text)));
                    }
                }
                0x1 => {
                    // Data frames never interleave a message in flight;
                    // control frames are the only ones that may.
                    if self.fragment.is_some() {
                        return Err(invalid("a new message opened while one is in flight"));
                    }
                    if fin {
                        return Ok(Some(Message::Text(utf8(payload)?)));
                    }
                    self.fragment = Some(payload);
                }
                0x8 if fin => return Ok(None),
                // Control frames may interleave a fragmented message (RFC
                // 6455); the ping or pong is answered out of the stream and
                // the fragment stays open.
                0x9 if fin => return Ok(Some(Message::Ping(payload))),
                0xA if fin => return Ok(Some(Message::Pong(payload))),
                _ => return Err(violation(opcode)),
            }
        }
    }
}

fn utf8(payload: Vec<u8>) -> io::Result<String> {
    String::from_utf8(payload).map_err(|_| invalid("a text frame must carry utf-8"))
}

fn violation(opcode: u8) -> io::Error {
    invalid(match opcode {
        0x2 => "binary frames carry no part of the protocol",
        _ => "an unexpected frame opcode",
    })
}

fn too_large() -> io::Error {
    invalid("message too large")
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

async fn read_frame(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<(bool, u8, Vec<u8>)> {
    let header = reader.read_u16().await?;
    let fin = header & 0x8000 != 0;
    if header & 0x7000 != 0 {
        return Err(invalid("reserved frame bits set"));
    }
    let opcode = (header >> 8) as u8 & 0x0F;
    if header & 0x0080 == 0 {
        return Err(invalid("client frames must be masked"));
    }
    let length = match header & 0x007F {
        126 => reader.read_u16().await? as usize,
        127 => {
            let length = reader.read_u64().await?;
            if length > MAX_MESSAGE as u64 {
                return Err(too_large());
            }
            length as usize
        }
        length => length as usize,
    };
    if length > MAX_MESSAGE {
        return Err(too_large());
    }
    let mut mask = [0u8; 4];
    reader.read_exact(&mut mask).await?;
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % 4];
    }
    Ok((fin, opcode, payload))
}

/// Write one text message.
pub async fn write_text(writer: &mut (impl AsyncWrite + Unpin), text: &str) -> io::Result<()> {
    write_frame(writer, 0x1, text.as_bytes()).await
}

/// Answer a ping.
pub async fn write_pong(writer: &mut (impl AsyncWrite + Unpin), payload: &[u8]) -> io::Result<()> {
    write_frame(writer, 0xA, payload).await
}

async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    opcode: u8,
    payload: &[u8],
) -> io::Result<()> {
    let mut frame = vec![0x80 | opcode];
    match payload.len() {
        0..=125 => frame.push(payload.len() as u8),
        126..=65_535 => {
            frame.push(126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        _ => {
            frame.push(127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    writer.write_all(&frame).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    /// One masked client frame, its mask zeros so the payload rides as-is.
    fn frame(fin: bool, opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![
            (if fin { 0x80 } else { 0 }) | opcode,
            0x80 | payload.len() as u8,
            0,
            0,
            0,
            0,
        ];
        out.extend_from_slice(payload);
        out
    }

    async fn read(bytes: &[u8]) -> io::Result<Option<Message>> {
        Reader::new().read(&mut Cursor::new(bytes)).await
    }

    #[tokio::test]
    async fn a_fragmented_message_completes_from_its_continuation() {
        let mut bytes = frame(false, 0x1, b"hello ");
        bytes.extend(frame(true, 0x0, b"world"));
        assert_eq!(
            read(&bytes).await.unwrap(),
            Some(Message::Text("hello world".into()))
        );
    }

    #[tokio::test]
    async fn a_ping_during_a_fragmented_message_comes_out_before_the_message_completes() {
        let mut bytes = frame(false, 0x1, b"hello");
        bytes.extend(frame(true, 0x9, b"keepalive"));
        bytes.extend(frame(true, 0x0, b" world"));
        let mut stream = Reader::new();
        let mut bytes = &bytes[..];
        assert_eq!(
            stream.read(&mut bytes).await.unwrap(),
            Some(Message::Ping(b"keepalive".to_vec()))
        );
        assert_eq!(
            stream.read(&mut bytes).await.unwrap(),
            Some(Message::Text("hello world".into()))
        );
    }

    #[tokio::test]
    async fn a_continuation_without_a_message_open_is_an_error() {
        let error = read(&frame(true, 0x0, b"stray")).await.unwrap_err();
        assert!(error.to_string().contains("no message open"), "{error}");
    }

    #[tokio::test]
    async fn a_new_text_frame_while_a_message_is_in_flight_is_an_error() {
        let mut bytes = frame(false, 0x1, b"open");
        bytes.extend(frame(true, 0x1, b"again"));
        let error = read(&bytes).await.unwrap_err();
        assert!(
            error.to_string().contains("while one is in flight"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn an_unmasked_frame_is_an_error() {
        let bytes = [0x81, 0x00];
        let error = read(&bytes).await.unwrap_err();
        assert!(error.to_string().contains("must be masked"), "{error}");
    }

    #[tokio::test]
    async fn reserved_bits_set_are_an_error() {
        let mut bytes = frame(true, 0x1, b"hi");
        bytes[0] |= 0x40;
        let error = read(&bytes).await.unwrap_err();
        assert!(error.to_string().contains("reserved"), "{error}");
    }

    #[tokio::test]
    async fn a_frame_beyond_the_size_cap_is_an_error() {
        // The 64-bit extended length declares more than the cap allows, so
        // the error comes before any payload is read.
        let mut bytes = vec![0x81, 0x80 | 127];
        bytes.extend(((MAX_MESSAGE as u64) + 1).to_be_bytes());
        let error = read(&bytes).await.unwrap_err();
        assert!(error.to_string().contains("too large"), "{error}");
    }
}
