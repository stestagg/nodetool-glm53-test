//! The editor server: the envelope's round trip, the greeting, the listing
//! and definition requests, the create, move, wire, unhook, and delete
//! operations against the held definition, every malformed path, and the
//! served address end to end over real sockets.

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

/// Create one node at a fixed spot, answering its uuid: the setup step the
/// wire, unhook, and delete tests build graphs from, positions being
/// incidental to them.
fn create(editor: &Editor, id: u64, type_ref: &str) -> String {
    send(
        editor,
        &format!(
            r#"{{"id": {id}, "type": "create_node", "type_ref": "{type_ref}", "position": {{"x": 0, "y": 0}}}}"#
        ),
    )["uuid"]
        .as_str()
        .unwrap()
        .to_owned()
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
    let mut watchers = [editor.subscribe(), editor.subscribe()];

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

    // The push reaches every connection, each with the whole updated
    // definition.
    for watcher in &mut watchers {
        let pushed = push(watcher);
        assert_eq!(pushed["type"], "definition");
        assert_eq!(
            pushed["graph"]["nodes"][0]["metadata"]["position"],
            json!({ "x": 40.5, "y": 2 })
        );
    }
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
fn wire_creates_the_edge_and_pushes_it_to_every_connection() {
    let editor = editor();
    let a = create(&editor, 1, "alpha/add");
    let b = create(&editor, 2, "beta/identity");
    let mut watchers = [editor.subscribe(), editor.subscribe()];

    let wired = send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "{a}", "from_port": "sum", "to": "{b}", "to_port": "value"}}"#
        ),
    );
    assert_eq!(wired["type"], "wired");
    assert_eq!(wired["id"], 3);

    let definition = held_definition(&editor);
    assert_eq!(
        definition["edges"],
        json!([{
            "from": a,
            "from_port": "sum",
            "to": b,
            "to_port": "value",
        }]),
        "the edge lands in the definition exactly as the operation named it"
    );

    // The push reaches every connection, each with the whole updated
    // definition.
    for watcher in &mut watchers {
        let pushed = push(watcher);
        assert_eq!(pushed["type"], "definition");
        assert_eq!(pushed["graph"]["edges"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn one_output_feeds_any_number_of_inputs() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let mut edges = Vec::new();
    for id in [2, 3] {
        let identity = create(&editor, id, "beta/identity");
        send(
            &editor,
            &format!(
                r#"{{"id": {id}, "type": "wire", "from": "{adder}", "from_port": "sum", "to": "{identity}", "to_port": "value"}}"#
            ),
        );
        edges.push(json!({
            "from": adder,
            "from_port": "sum",
            "to": identity,
            "to_port": "value",
        }));
    }
    assert_eq!(
        held_definition(&editor)["edges"],
        json!(edges),
        "each downstream input carries its own wire"
    );
}

#[test]
fn a_wire_on_an_occupied_input_replaces_the_old_wire_and_any_literal() {
    let editor = editor_holding(SEEDED);
    let adder = "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33";
    let identity = create(&editor, 1, "beta/identity");
    let tick = create(&editor, 2, "beta/tick");

    // The adder's `a` input holds the literal 2; the first wire replaces it.
    send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "{identity}", "from_port": "value", "to": "{adder}", "to_port": "a"}}"#
        ),
    );
    let definition = held_definition(&editor);
    let adder_node = &definition["nodes"][0];
    assert_eq!(
        definition["edges"],
        json!([{ "from": identity, "from_port": "value", "to": adder, "to_port": "a" }]),
    );
    assert!(
        adder_node.get("parameters").is_none(),
        "an input carries a connection or a literal, never both: {adder_node}"
    );

    // The second wire replaces the first on the same input.
    send(
        &editor,
        &format!(
            r#"{{"id": 4, "type": "wire", "from": "{tick}", "from_port": "tick", "to": "{adder}", "to_port": "a"}}"#
        ),
    );
    let definition = held_definition(&editor);
    assert_eq!(
        definition["edges"],
        json!([{ "from": tick, "from_port": "tick", "to": adder, "to_port": "a" }]),
        "the input's old upstream is gone everywhere"
    );
}

#[test]
fn a_wire_from_a_nodes_output_to_its_own_input_lands() {
    let editor = editor();
    let identity = create(&editor, 1, "beta/identity");

    let wired = send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "wire", "from": "{identity}", "from_port": "value", "to": "{identity}", "to_port": "value"}}"#
        ),
    );
    assert_eq!(wired["type"], "wired", "edit time rejects no wire");
    assert_eq!(
        held_definition(&editor)["edges"],
        json!([{ "from": identity, "from_port": "value", "to": identity, "to_port": "value" }]),
        "compile time is what judges cycles, when a run starts"
    );
}

#[test]
fn unhook_removes_the_edge_and_is_idempotent() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let identity = create(&editor, 2, "beta/identity");
    send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "{identity}", "from_port": "value", "to": "{adder}", "to_port": "a"}}"#
        ),
    );

    let unhooked = send(
        &editor,
        &format!(r#"{{"id": 4, "type": "unhook", "to": "{adder}", "to_port": "a"}}"#),
    );
    assert_eq!(unhooked["type"], "unhooked");
    assert_eq!(unhooked["id"], 4);
    assert_eq!(
        held_definition(&editor)["edges"],
        json!([]),
        "the input returns to unconnected"
    );

    // Unhooking again — or an input that never carried a wire — changes
    // nothing and answers the same.
    let again = send(
        &editor,
        &format!(r#"{{"id": 5, "type": "unhook", "to": "{adder}", "to_port": "b"}}"#),
    );
    assert_eq!(again["type"], "unhooked");
    assert_eq!(
        held_definition(&editor)["nodes"].as_array().unwrap().len(),
        2,
        "unhooking never touches nodes"
    );
}

#[test]
fn delete_removes_the_node_and_cascades_its_edges() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let identity = create(&editor, 2, "beta/identity");
    let tick = create(&editor, 3, "beta/tick");
    send(
        &editor,
        &format!(
            r#"{{"id": 4, "type": "wire", "from": "{adder}", "from_port": "sum", "to": "{identity}", "to_port": "value"}}"#
        ),
    );
    send(
        &editor,
        &format!(
            r#"{{"id": 5, "type": "wire", "from": "{tick}", "from_port": "tick", "to": "{adder}", "to_port": "b"}}"#
        ),
    );

    let deleted = send(
        &editor,
        &format!(r#"{{"id": 6, "type": "delete_node", "uuid": "{adder}"}}"#),
    );
    assert_eq!(deleted["type"], "node_deleted");
    assert_eq!(deleted["id"], 6);

    let definition = held_definition(&editor);
    let uuids: Vec<_> = definition["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["uuid"].clone())
        .collect();
    assert_eq!(
        json!(uuids),
        json!([identity, tick]),
        "only the deleted node goes"
    );
    assert_eq!(
        definition["edges"],
        json!([]),
        "no dangling edges are left behind"
    );
}

#[test]
fn wire_unhook_and_delete_naming_an_unknown_node_are_errors() {
    let editor = editor_holding(SEEDED);
    let before = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);
    let unknown = "0d5c1e2a-3b4c-4d5e-8f90-1a2b3c4d5e6f";

    let wired = send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "wire", "from": "{unknown}", "from_port": "sum", "to": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "to_port": "a"}}"#
        ),
    );
    assert_eq!(wired["type"], "error");
    assert!(
        wired["error"].as_str().unwrap().contains("no node"),
        "the error names the problem: {wired}"
    );

    let landed = send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "from_port": "sum", "to": "{unknown}", "to_port": "a"}}"#
        ),
    );
    assert_eq!(landed["type"], "error");

    let unhooked = send(
        &editor,
        &format!(r#"{{"id": 4, "type": "unhook", "to": "{unknown}", "to_port": "a"}}"#),
    );
    assert_eq!(unhooked["type"], "error");

    let deleted = send(
        &editor,
        &format!(r#"{{"id": 5, "type": "delete_node", "uuid": "{unknown}"}}"#),
    );
    assert_eq!(deleted["type"], "error");

    let after = send(&editor, r#"{"id": 6, "type": "get_definition"}"#);
    assert_eq!(
        before["graph"], after["graph"],
        "the definition is untouched"
    );
    assert_eq!(after["id"], 6, "the connection is still usable");
}

#[test]
fn malformed_wire_unhook_and_delete_are_errors_and_leave_the_connection_usable() {
    let editor = editor();

    for (message, id) in [
        (r#"{"id": 1, "type": "wire"}"#, 1),
        (
            r#"{"id": 2, "type": "wire", "from": "x", "from_port": "sum", "to": "y", "to_port": "a"}"#,
            2,
        ),
        (
            r#"{"id": 3, "type": "wire", "from": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "from_port": 1, "to": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "to_port": "a"}"#,
            3,
        ),
        (
            r#"{"id": 4, "type": "wire", "from": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "from_port": "sum", "to": "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33", "to_port": "a", "extra": true}"#,
            4,
        ),
        (r#"{"id": 5, "type": "unhook", "to": "not a uuid"}"#, 5),
        (r#"{"id": 6, "type": "unhook"}"#, 6),
        (r#"{"id": 7, "type": "delete_node", "uuid": 9}"#, 7),
        (r#"{"id": 8, "type": "delete_node"}"#, 8),
    ] {
        let reply = send(&editor, message);
        assert_eq!(reply["type"], "error", "for {message}");
        assert_eq!(reply["id"], id, "for {message}");
    }

    let usable = send(&editor, r#"{"id": 9, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 9);
    assert_eq!(usable["graph"]["edges"], json!([]));
    assert_eq!(
        usable["graph"]["nodes"].as_array().unwrap().len(),
        0,
        "nothing landed"
    );
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
