//! The editor server: the envelope's round trip, the greeting, the listing
//! and definition requests, the create and move operations against the held
//! definition, every malformed path, and the served address end to end over
//! real sockets.

use std::sync::Arc;

use nodetool::graph;
use nodetool::registry;
use nodetool::server::Editor;
use serde_json::{json, Value};
use test_plugin_alpha as _;
use test_plugin_beta as _;

fn editor() -> Editor {
    Editor::new(graph::GraphDefinition::empty())
}

fn editor_holding(graph: &str) -> Editor {
    Editor::new(graph::load(graph).expect("the test seeds a loadable definition"))
}

fn send(editor: &Editor, message: &str) -> Value {
    serde_json::from_str(&editor.handle(message)).expect("every reply is a JSON object")
}

fn push(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> Value {
    serde_json::from_str(&receiver.blocking_recv().expect("a push arrives"))
        .expect("a push is JSON")
}

fn held_definition(editor: &Editor) -> Value {
    send(editor, r#"{"id": 0, "type": "get_definition"}"#)["graph"].clone()
}

const SEEDED: &str = "schema_version: 1
name: seeded
nodes:
  - uuid: b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33
    type_ref: alpha/add
    label: The adder
    parameters:
      a: 2
    metadata:
      position: { x: 80, y: 120 }
      color: \"#4a90d9\"
edges: []";

#[test]
fn greeting_names_both_versions() {
    let greeting: Value = serde_json::from_str(&nodetool::server::greeting()).unwrap();
    assert_eq!(greeting["type"], "greeting");
    assert_eq!(greeting["protocol_version"], 1);
    assert_eq!(greeting["schema_version"], graph::SCHEMA_VERSION);
}

#[test]
fn a_request_is_answered_with_the_matching_id() {
    let editor = editor();
    let reply = send(&editor, r#"{"id": 7, "type": "get_definition"}"#);
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["type"], "definition");
    assert_eq!(reply["graph"]["schema_version"], 1);
    assert_eq!(reply["graph"]["nodes"], json!([]));
    assert_eq!(reply["graph"]["edges"], json!([]));
}

#[test]
fn the_definition_reply_carries_the_held_definition_verbatim() {
    let editor = editor_holding(SEEDED);
    let reply = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);
    assert_eq!(reply["graph"]["name"], "seeded");
    let node = &reply["graph"]["nodes"][0];
    assert_eq!(node["uuid"], "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33");
    assert_eq!(node["type_ref"], "alpha/add");
    assert_eq!(node["label"], "The adder");
    assert_eq!(node["parameters"], json!({ "a": 2 }));
    assert_eq!(node["metadata"]["position"], json!({ "x": 80, "y": 120 }));
    assert_eq!(node["metadata"]["color"], "#4a90d9");
}

#[test]
fn the_listing_matches_the_registry() {
    let editor = editor();
    let reply = send(&editor, r#"{"id": 2, "type": "list_node_types"}"#);
    assert_eq!(reply["type"], "node_types");
    let listed: Vec<&str> = reply["node_types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node_type| node_type["type_ref"].as_str().unwrap())
        .collect();
    let mut registered = registry::node_types()
        .map(|node_type| node_type.type_ref)
        .collect::<Vec<_>>();
    registered.sort_unstable();
    assert_eq!(
        listed, registered,
        "the listing is the registry, in a stable order"
    );

    let add = reply["node_types"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node_type| node_type["type_ref"] == "alpha/add")
        .unwrap();
    assert_eq!(add["label"], "Add");
    assert_eq!(add["plugin"], "alpha");
    assert_eq!(add["sub_group"], "math");
    assert_eq!(
        add["inputs"],
        json!([{ "name": "a", "type_refs": ["i32"] }, { "name": "b", "type_refs": ["i32"] }])
    );
    assert_eq!(
        add["outputs"],
        json!([{ "name": "sum", "type_refs": ["i32"] }])
    );
}

#[test]
fn create_assigns_a_fresh_uuid_and_records_the_position() {
    let editor = editor();
    let mut watcher = editor.subscribe();
    let first = send(
        &editor,
        r#"{"id": 1, "type": "create_node", "type_ref": "alpha/add", "position": {"x": 10, "y": -4}}"#,
    );
    assert_eq!(first["type"], "node_created");
    assert_eq!(first["id"], 1);
    let second = send(
        &editor,
        r#"{"id": 2, "type": "create_node", "type_ref": "alpha/add", "position": {"x": 200, "y": 60}}"#,
    );
    assert_ne!(
        first["uuid"], second["uuid"],
        "every instance gets its own uuid"
    );

    let definition = held_definition(&editor);
    let nodes = definition["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["type_ref"], "alpha/add");
    assert!(
        nodes[0].get("label").is_none(),
        "a fresh node carries no label override"
    );
    assert_eq!(
        nodes[0]["metadata"]["position"],
        json!({ "x": 10, "y": -4 }),
        "the drop position is recorded in the node's metadata"
    );

    // Each create pushed the whole updated definition, the same message
    // every connection receives.
    let pushed = push(&mut watcher);
    assert_eq!(pushed["type"], "definition");
    let nodes = pushed["graph"]["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0]["metadata"]["position"],
        json!({ "x": 10, "y": -4 })
    );
    let pushed = push(&mut watcher);
    assert_eq!(pushed["graph"]["nodes"].as_array().unwrap().len(), 2);
}

#[test]
fn create_naming_an_unlinked_type_is_an_error_and_the_definition_is_untouched() {
    let editor = editor();
    let reply = send(
        &editor,
        r#"{"id": 5, "type": "create_node", "type_ref": "shapes/circle", "position": {"x": 0, "y": 0}}"#,
    );
    assert_eq!(reply["type"], "error");
    assert_eq!(reply["id"], 5);
    let message = reply["error"].as_str().unwrap();
    assert!(
        message.contains("shapes/circle"),
        "the error names the problem: {message}"
    );

    let still_empty = send(&editor, r#"{"id": 6, "type": "get_definition"}"#);
    assert_eq!(still_empty["id"], 6, "the connection is still usable");
    assert_eq!(still_empty["graph"]["nodes"], json!([]));
}

#[test]
fn move_records_the_resting_position_in_metadata() {
    let editor = editor();
    let created = send(
        &editor,
        r#"{"id": 1, "type": "create_node", "type_ref": "beta/identity", "position": {"x": 1, "y": 2}}"#,
    );
    let uuid = created["uuid"].as_str().unwrap();
    let mut watcher = editor.subscribe();

    let moved = send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "move_node", "uuid": "{uuid}", "position": {{"x": 40.5, "y": 2}}}}"#
        ),
    );
    assert_eq!(moved["type"], "node_moved");
    assert_eq!(moved["id"], 2);

    let definition = held_definition(&editor);
    assert_eq!(
        definition["nodes"][0]["metadata"]["position"],
        json!({ "x": 40.5, "y": 2 }),
        "the move updates the node's metadata position, keeping each coordinate's kind"
    );

    let pushed = push(&mut watcher);
    assert_eq!(pushed["type"], "definition");
    assert_eq!(
        pushed["graph"]["nodes"][0]["metadata"]["position"],
        json!({ "x": 40.5, "y": 2 })
    );
}

#[test]
fn a_move_leaves_the_nodes_other_metadata_alone() {
    let editor = editor_holding(SEEDED);
    send(
        &editor,
        r#"{"id": 1, "type": "move_node", "uuid": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "position": {"x": 7, "y": 8}}"#,
    );
    let metadata = &held_definition(&editor)["nodes"][0]["metadata"];
    assert_eq!(
        metadata["color"], "#4a90d9",
        "the node's own bookkeeping stays"
    );
    assert_eq!(metadata["position"], json!({ "x": 7, "y": 8 }));
}

#[test]
fn move_naming_an_unknown_uuid_is_an_error_and_the_definition_is_untouched() {
    let editor = editor_holding(SEEDED);
    let before = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);

    let unknown = send(
        &editor,
        r#"{"id": 2, "type": "move_node", "uuid": "0d5c1e2a-3b4c-4d5e-8f90-1a2b3c4d5e6f", "position": {"x": 9, "y": 9}}"#,
    );
    assert_eq!(unknown["type"], "error");
    assert!(unknown["error"].as_str().unwrap().contains("no node"));

    let malformed = send(
        &editor,
        r#"{"id": 3, "type": "move_node", "uuid": "not a uuid", "position": {"x": 9, "y": 9}}"#,
    );
    assert_eq!(malformed["type"], "error");

    let after = send(&editor, r#"{"id": 4, "type": "get_definition"}"#);
    assert_eq!(
        before["graph"], after["graph"],
        "the definition is untouched"
    );
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

#[test]
fn pushes_reach_every_connection() {
    let editor = editor();
    let mut one = editor.subscribe();
    let mut two = editor.subscribe();
    editor.broadcast(&json!({ "type": "definition", "graph": graph::GraphDefinition::empty() }));
    for receiver in [&mut one, &mut two] {
        assert_eq!(push(receiver)["type"], "definition");
    }
}

#[test]
fn the_default_address_is_loopback() {
    let (host, port) = nodetool::server::DEFAULT_ADDRESS
        .rsplit_once(':')
        .expect("the default address carries a port");
    assert_eq!(host, "127.0.0.1", "a local tool binds loopback");
    assert!(
        port.parse::<u16>().is_ok_and(|port| port >= 1024),
        "the default port is an unprivileged port"
    );
}

// The served address end to end: the editor shell over HTTP, the upgrade to
// the websocket endpoint, the greeting push, and one request answered with
// the id it echoed.
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

#[test]
fn a_created_node_is_a_definition_node_the_compiler_takes() {
    let editor = editor();
    send(
        &editor,
        r#"{"id": 1, "type": "create_node", "type_ref": "alpha/add", "position": {"x": 5, "y": 6}}"#,
    );
    let definition = held_definition(&editor);
    assert_eq!(definition["schema_version"], graph::SCHEMA_VERSION);
    assert!(definition["nodes"][0].get("parameters").is_none());
    assert_eq!(definition["edges"], json!([]));
}
