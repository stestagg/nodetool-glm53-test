//! The websocket transport, server side: the handshake answer and the
//! frame codec the protocol rides on. Text frames carry the JSON protocol;
//! ping, pong, and close are the only other frames a peer may send —
//! anything else is a protocol violation that ends the connection, because
//! this server speaks with browsers it serves itself and never negotiates
//! extensions.

use std::io;

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

/// The `Sec-WebSocket-Accept` answer for a client's handshake key.
pub fn accept_key(client_key: &str) -> String {
    base64(&sha1(format!("{client_key}{HANDSHAKE_GUID}").as_bytes()))
}

/// Read one message. Returns `None` when the peer closed the connection.
///
/// A message that violates the framing rules — an unmasked frame, a
/// reserved bit, a frame over the size cap, a bad opcode — is an error that
/// ends the connection; the caller responds by dropping it.
pub async fn read_message(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Option<Message>> {
    let mut fragment: Option<Vec<u8>> = None;
    loop {
        let (fin, opcode, payload) = read_frame(reader).await?;
        match opcode {
            0x1 if fin => return Ok(Some(Message::Text(utf8(payload)?))),
            0x1 => match fragment {
                None => fragment = Some(payload),
                Some(_) => return Err(invalid("a new message opened while one is in flight")),
            },
            0x0 => {
                let accumulated = fragment
                    .as_mut()
                    .ok_or_else(|| invalid("a continuation frame with no message open"))?;
                accumulated.extend_from_slice(&payload);
                if accumulated.len() > MAX_MESSAGE {
                    return Err(too_large());
                }
                if fin {
                    let text = utf8(
                        fragment
                            .take()
                            .expect("a continuation implies a message open"),
                    )?;
                    return Ok(Some(Message::Text(text)));
                }
            }
            0x8 if fin => return Ok(None),
            0x9 if fin => return Ok(Some(Message::Ping(payload))),
            0xA if fin => return Ok(Some(Message::Pong(payload))),
            _ => return Err(violation(opcode)),
        }
    }
}

fn utf8(payload: Vec<u8>) -> io::Result<String> {
    String::from_utf8(payload).map_err(|_| invalid("a text frame must carry utf-8"))
}

fn violation(opcode: u8) -> io::Error {
    invalid(match opcode {
        0x0 => "a continuation frame with no message open",
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

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let bit_length = (data.len() as u64) * 8;
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    for block in message.as_chunks::<64>().0 {
        let mut words = [0u32; 80];
        for (index, word) in block.as_chunks::<4>().0.iter().enumerate() {
            words[index] = u32::from_be_bytes(*word);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) =
            (state[0], state[1], state[2], state[3], state[4]);
        for (index, word) in words.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let step = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = step;
        }
        for (state, round) in state.iter_mut().zip([a, b, c, d, e]) {
            *state = state.wrapping_add(round);
        }
    }
    let mut digest = [0u8; 20];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..][..4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(data: &[u8]) -> String {
    let mut encoded = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let bytes = [
            0,
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let word = u32::from_be_bytes(bytes);
        for (index, sextet) in [
            (word >> 18) & 0x3F,
            (word >> 12) & 0x3F,
            (word >> 6) & 0x3F,
            word & 0x3F,
        ]
        .iter()
        .enumerate()
        {
            if index <= chunk.len() {
                encoded.push(BASE64[*sextet as usize] as char);
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}
