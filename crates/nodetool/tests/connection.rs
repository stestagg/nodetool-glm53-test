//! The transport the protocol rides: the greeting, the envelope's id
//! echo and its malformed-message paths, and the served address end to
//! end over a real socket. The operations against the held definition
//! are server.rs's.

use std::sync::Arc;

use nodetool::graph;
use nodetool::server::{greeting, Editor, DEFAULT_ADDRESS};
use serde_json::{json, Value};

fn editor() -> Editor {
    Editor::new(graph::GraphDefinition::empty(), None)
}

fn send(editor: &Editor, message: &str) -> Value {
    serde_json::from_str(&editor.handle(message)).expect("every reply is a JSON object")
}

#[test]
fn greeting_names_both_versions() {
    let greeting: Value = serde_json::from_str(&greeting()).unwrap();
    assert_eq!(greeting["type"], "greeting");
    assert_eq!(greeting["protocol_version"], 1);
    assert_eq!(greeting["schema_version"], graph::SCHEMA_VERSION);
}

#[test]
fn the_default_address_is_a_loopback_one() {
    let address: std::net::SocketAddr = DEFAULT_ADDRESS
        .parse()
        .expect("the default address is a socket address the server can bind");
    assert!(
        address.ip().is_loopback(),
        "the default is loopback: this is a local tool, not one served across a boundary — {address}"
    );
}

#[test]
fn a_request_is_answered_with_the_matching_id() {
    let editor = editor();
    let reply = send(&editor, r#"{"id": 7, "type": "get_definition"}"#);
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["type"], "definition");
    assert_eq!(reply["graph"]["schema_version"], graph::SCHEMA_VERSION);
    assert_eq!(reply["graph"]["nodes"], json!([]));
    assert_eq!(reply["graph"]["edges"], json!([]));
}

#[test]
fn malformed_input_is_answered_with_an_error_and_leaves_the_connection_usable() {
    let editor = editor();

    for (message, id) in [
        ("not json at all", None),
        ("[1, 2]", None),
        (r#""a string""#, None),
        (r#"{"type": "get_definition"}"#, None), // no id to echo
        (r#"{"id": 1}"#, Some(1)),               // no type, id echoed
        (r#"{"id": 1, "type": 2}"#, Some(1)),    // type not a string, id echoed
        (r#"{"id": 1, "type": "nope"}"#, Some(1)), // unknown type, id echoed
        (
            r#"{"id": 2, "type": "get_definition", "surprise": true}"#,
            Some(2),
        ),
        (
            r#"{"id": 3, "type": "create_node", "type_ref": "alpha/add", "position": {"x": 1, "y": 2, "z": 3}}"#,
            Some(3),
        ),
        (
            r#"{"id": 4, "type": "create_node", "type_ref": "alpha/add", "position": {"x": "left", "y": 2}}"#,
            Some(4),
        ),
        (
            r#"{"id": 5, "type": "create_node", "type_ref": "alpha/add"}"#,
            Some(5),
        ),
    ] {
        let reply = send(&editor, message);
        assert_eq!(reply["type"], "error", "for {message}");
        match id {
            Some(id) => assert_eq!(reply["id"], id, "for {message}"),
            None => assert!(reply["id"].is_null(), "no id to echo, for {message}"),
        }
    }

    let usable = send(&editor, r#"{"id": 9, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 9);
    assert_eq!(usable["graph"]["nodes"], json!([]));
}

#[tokio::test]
async fn serves_the_page_and_answers_over_the_websocket() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let editor = Arc::new(editor());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(Arc::clone(&editor).serve(listener));

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: editor\r\n\r\n")
        .await
        .unwrap();
    let mut page = String::new();
    stream.read_to_string(&mut page).await.unwrap();
    assert!(page.starts_with("HTTP/1.1 200 OK\r\n"), "for GET /: {page}");
    assert!(page.contains("Content-Type: text/html"));
    assert!(
        page.contains(r#"<div id="root"></div>"#),
        "the editor shell: {page}"
    );

    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            b"GET /ws HTTP/1.1\r\nHost: editor\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        )
        .await
        .unwrap();

    // The greeting push may ride the same read as the upgrade answer, so
    // everything the socket delivers is carried in one buffer and read
    // incrementally: first the 101 answer, then the frames.
    let mut buffer = Vec::new();
    let upgrade_head = loop {
        if let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break String::from_utf8_lossy(&buffer[..end + 4]).into_owned();
        }
        let mut chunk = [0u8; 1024];
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the connection closed before the upgrade");
        buffer.extend_from_slice(&chunk[..read]);
    };
    assert!(
        upgrade_head.starts_with("HTTP/1.1 101 Switching Protocols\r\n"),
        "{upgrade_head}"
    );
    assert!(
        upgrade_head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
        "the RFC 6455 example answer: {upgrade_head}"
    );

    // One request rides a masked client frame; the greeting push arrives
    // first, then the reply echoes the request's id.
    let request = br#"{"id": 3, "type": "get_definition"}"#;
    let mut frame = vec![0x81];
    frame.push(0x80 | request.len() as u8); // client frames are masked
    frame.extend_from_slice(&[0, 0, 0, 0]);
    frame.extend_from_slice(request);
    stream.write_all(&frame).await.unwrap();

    loop {
        let text = String::from_utf8_lossy(&buffer);
        if text.contains(r#""type":"greeting""#) && text.contains(r#""id":3"#) {
            break;
        }
        let mut chunk = [0u8; 1024];
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "the connection closed before the reply: {text}");
        buffer.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8_lossy(&buffer);
    assert!(
        text.contains(r#""type":"definition""#),
        "the request is answered: {text}"
    );

    server.abort();
}
