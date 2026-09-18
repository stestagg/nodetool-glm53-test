//! The sliver of HTTP the editor needs: one GET that serves the embedded UI
//! assets, one upgrade to the websocket endpoint. This is not a general web
//! server — anything else answers with an error and the connection closes.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The largest request head the server reads. A bigger one ends the
/// connection before any of it is parsed.
const MAX_HEAD: usize = 16 * 1024;

/// One request head, parsed just far enough to route it: the method and
/// path, and the websocket key when the request asks for an upgrade.
pub struct Request {
    pub method: String,
    pub path: String,
    pub websocket_key: Option<String>,
}

/// Read one request head. An unparseable or oversized head is an error; the
/// caller answers with a 400 and closes.
pub async fn read_request(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Request> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if head.len() >= MAX_HEAD {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
        if reader.read(&mut byte).await? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "connection closed mid-request",
            ));
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    parse_request(
        &String::from_utf8(head).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "request head must be utf-8")
        })?,
    )
}

fn parse_request(head: &str) -> io::Result<Request> {
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let (method, path, version) = (parts.next(), parts.next(), parts.next());
    let (method, path) = match (method, path, version) {
        (Some(method), Some(path), Some(version)) if version.starts_with("HTTP/") => {
            (method.to_owned(), path.to_owned())
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed request line",
            ))
        }
    };
    let mut websocket_key = None;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed header line",
            ));
        };
        if name.trim().eq_ignore_ascii_case("sec-websocket-key") {
            websocket_key = Some(value.trim().to_owned());
        }
    }
    Ok(Request {
        method,
        path,
        websocket_key,
    })
}

/// Write one complete response: head and body, then the connection closes.
/// The served assets are embedded and change only with a rebuild, so the
/// browser must revalidate before every use: a rebuilt binary and a reload
/// is the whole update path.
pub async fn write_response(
    writer: &mut (impl AsyncWrite + Unpin),
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write_head(
        writer,
        &format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
            body.len()
        ),
    )
    .await?;
    writer.write_all(body).await?;
    writer.flush().await
}

/// Write the websocket upgrade answer for a computed accept key.
pub async fn write_upgrade(
    writer: &mut (impl AsyncWrite + Unpin),
    accept_key: &str,
) -> io::Result<()> {
    write_head(
        writer,
        &format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept_key}\r\n\r\n"
        ),
    )
    .await
}

async fn write_head(writer: &mut (impl AsyncWrite + Unpin), head: &str) -> io::Result<()> {
    writer.write_all(head.as_bytes()).await?;
    writer.flush().await
}
