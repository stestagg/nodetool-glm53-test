//! The custom-UI path the editor serves: the entry-asset facts on the
//! node-type listing and the type facts — present for a declaring plugin,
//! absent for one without — the assets themselves under the per-plugin
//! path over a real socket, a missing asset answered as the error it is,
//! the serialiser invoked generically where the type's values cross the
//! bridge, and the resync carrying the serialised value to a tab that
//! joins late. The lock and the operation paths the plugin helpers ride
//! are server_run.rs's; the browser-side contract checks are the UI test
//! runner's.

mod server_common;

use std::sync::Arc;

use nodetool::server::Editor;
use serde_json::{json, Value};
use server_common::*;
use test_plugin_alpha as _;
use test_plugin_delta as _;
use test_plugin_epsilon as _;
use tokio::sync::broadcast;

const TONE_SOURCE: &str = "schema_version: 1
name: tone
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000f1
    type_ref: epsilon/source
edges: []";

#[test]
fn the_node_type_listing_carries_the_ui_fact_for_a_declared_type_alone() {
    let editor = editor();
    let reply = send(&editor, r#"{"id": 1, "type": "list_node_types"}"#);
    let listed = reply["node_types"].as_array().unwrap();
    let widget = listed
        .iter()
        .find(|node_type| node_type["type_ref"] == "epsilon/widget")
        .expect("the fixture is linked");
    assert_eq!(
        widget["ui"],
        json!({ "entry": "/plugins/epsilon/widget-node.js", "contract": 1 })
    );
    for other in ["epsilon/source", "epsilon/lamp_source", "alpha/add"] {
        let plain = listed
            .iter()
            .find(|node_type| node_type["type_ref"] == other)
            .unwrap();
        assert!(
            plain.get("ui").is_none(),
            "a plugin declaring no UI adds no fact: {plain}"
        );
    }
}

#[test]
fn the_type_facts_carry_the_ui_fact_for_a_serialiser_declaring_type_alone() {
    let editor = editor();
    let reply = send(&editor, r#"{"id": 1, "type": "list_node_types"}"#);
    let facts = &reply["data_types"];
    assert_eq!(
        facts["epsilon/tone"]["ui"],
        json!({ "entry": "/plugins/epsilon/tone-value.js", "contract": 1 })
    );
    assert!(
        facts["epsilon/lamp"].get("ui").is_none(),
        "no declaration, no fact: {facts}"
    );
    assert!(
        facts["i32"].get("ui").is_none(),
        "the base scalars declare no value UI: {facts}"
    );
    assert!(
        facts["alpha/ratio"].get("ui").is_none(),
        "a custom type without UI crosses contentless: {facts}"
    );
}

#[tokio::test]
async fn declared_assets_serve_under_the_per_plugin_path_and_a_missing_one_answers_404() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let editor = Arc::new(editor());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(Arc::clone(&editor).serve(listener));

    let get = |path: &'static str, address| {
        let request = format!("GET {path} HTTP/1.1\r\nHost: editor\r\n\r\n");
        async move {
            let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
            stream.write_all(request.as_bytes()).await.unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).await.unwrap();
            response
        }
    };

    let widget = get("/plugins/epsilon/widget-node.js", address).await;
    assert!(
        widget.starts_with("HTTP/1.1 200 OK\r\n"),
        "the declared bundle serves: {widget}"
    );
    assert!(widget.contains("Content-Type: text/javascript"));
    let source = nodetool::registry::node_type("epsilon/widget")
        .unwrap()
        .ui
        .unwrap()
        .source;
    assert!(
        widget.ends_with(source),
        "the body is the embedded bundle, verbatim: {widget}"
    );

    let tone = get("/plugins/epsilon/tone-value.js", address).await;
    assert!(
        tone.starts_with("HTTP/1.1 200 OK\r\n"),
        "the value bundle serves too"
    );

    let missing = get("/plugins/epsilon/missing.js", address).await;
    assert!(
        missing.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "a missing asset is the error it is, answered: {missing}"
    );

    server.abort();
}

/// Every push from here on, through the run's end: read until the run
/// state goes idle, the last push included.
async fn through_run_end(receiver: &mut broadcast::Receiver<String>) -> Vec<Value> {
    let mut pushed = Vec::new();
    loop {
        let message: Value = serde_json::from_str(&receiver.recv().await.expect("a push arrives"))
            .expect("a push is JSON");
        let ended = message["type"] == "run" && message["running"] == json!(false);
        pushed.push(message);
        if ended {
            return pushed;
        }
    }
}

/// The run display the connect-time resync delivers: asked for over the
/// one-message seam, arriving as its own push — the shape a connecting tab
/// actually receives it in.
async fn held_display(editor: &Editor) -> Value {
    let mut watcher = editor.subscribe();
    send(editor, r#"{"id": 0, "type": "get_definition"}"#);
    let push: Value = serde_json::from_str(&watcher.recv().await.expect("a push arrives"))
        .expect("a push is JSON");
    assert_eq!(push["type"], "run_display");
    push
}

#[tokio::test]
async fn the_serialiser_is_called_generically_when_its_types_values_cross_the_bridge() {
    let editor = editor_holding(TONE_SOURCE);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let pushes = through_run_end(&mut watcher).await;

    let emission = pushes
        .iter()
        .find(|push| push["event"] == "emitted")
        .expect("the source emits");
    assert_eq!(emission["port"], json!("tone"));
    assert_eq!(
        emission["value"],
        json!("440 Hz"),
        "the custom value crosses in the form its serialiser produces"
    );

    let snapshot = held_display(&editor).await;
    assert_eq!(
        snapshot["values"],
        json!({ "00000000-0000-0000-0000-0000000000f1/tone": "440 Hz" })
    );
}

#[tokio::test]
async fn a_type_with_no_declared_ui_still_crosses_contentless() {
    let editor = editor_holding(
        "schema_version: 1
name: lamp
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000f2
    type_ref: epsilon/lamp_source
edges: []",
    );
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let pushes = through_run_end(&mut watcher).await;

    let emission = pushes
        .iter()
        .find(|push| push["event"] == "emitted")
        .expect("the lamp source emits");
    assert!(
        emission.get("value").is_none(),
        "no serialiser, no invented content: {emission}"
    );
    let snapshot = held_display(&editor).await;
    assert_eq!(snapshot["values"], json!({}));
}

#[tokio::test]
async fn a_tab_joining_after_the_run_is_given_the_serialised_value_the_first_tab_holds() {
    let editor = editor_holding(TONE_SOURCE);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    through_run_end(&mut watcher).await;

    // A second tab connects after the run ended: the connect-time resync
    // carries the same display the first tab holds, serialised values
    // included.
    let mut late = editor.subscribe();
    send(&editor, r#"{"id": 2, "type": "get_definition"}"#);
    let display: Value =
        serde_json::from_str(&late.recv().await.expect("a push arrives")).expect("a push is JSON");
    assert_eq!(display["type"], "run_display");
    assert_eq!(
        display["values"],
        json!({ "00000000-0000-0000-0000-0000000000f1/tone": "440 Hz" })
    );
}
