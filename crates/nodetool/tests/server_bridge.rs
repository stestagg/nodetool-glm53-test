//! The bridge: the run's events forwarded to every connection as they
//! occur, the node statuses derived once server-side from those events —
//! story 07's closure rule, extended to the stopped outcome — the latest
//! value per emitting port held and riding the connect-time resync, and
//! runs that never wait on a connection: a stalled or closed one leaves
//! the run's values, timing, and completion untouched, and one that
//! cannot keep up is dropped over its real socket.

mod server_common;

use std::sync::Arc;

use nodetool::server::Editor;
use serde_json::{json, Value};
use server_common::*;
use test_plugin_delta as _;
use tokio::sync::broadcast;

const COUNTER_ALONE: &str = "schema_version: 1
name: counter
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
edges: []";

/// A run that ends on a node's error: the failer fires on the counter's
/// first value, while the pairer — its `b` fed by nothing — can never
/// conclude, so the closure rule has a started node to close as stopped.
const FAILING: &str = "schema_version: 1
name: failing
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000d2
    type_ref: delta/pairer
  - uuid: 00000000-0000-0000-0000-0000000000d5
    type_ref: delta/failer
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000d2
    to_port: a
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000d5
    to_port: value";

/// A run whose two nodes each emit through the other's pace — more pushes
/// than a connection's outbound bound holds.
const PAIRED: &str = "schema_version: 1
name: paired
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000d2
    type_ref: delta/pairer
    parameters:
      b: 1
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000d2
    to_port: a";

/// A graph whose run pushes more than a stalled connection's socket can
/// hold: the counter fans out to `width` pairers, each emitting once per
/// value it receives, so the run's pushes run past any buffer a
/// non-reading connection's kernel holds.
fn fan_out(width: usize) -> String {
    let mut graph = String::from(
        "schema_version: 1\nname: fan out\nnodes:\n  - uuid: 00000000-0000-0000-0000-0000000000d1\n    type_ref: delta/counter\n",
    );
    for index in 0..width {
        graph.push_str(&format!(
            "  - uuid: 00000000-0000-0000-0000-{:012x}\n    type_ref: delta/pairer\n    parameters:\n      b: 1\n",
            0x0000000000e0u32 + index as u32,
        ));
    }
    graph.push_str("edges:\n");
    for index in 0..width {
        graph.push_str(&format!(
            "  - from: 00000000-0000-0000-0000-0000000000d1\n    from_port: out\n    to: 00000000-0000-0000-0000-{:012x}\n    to_port: a\n",
            0x0000000000e0u32 + index as u32,
        ));
    }
    graph
}

/// A run that never ends by itself: the pairer's second input is neither
/// connected nor parameterised, so its gate never opens — the hang a stop
/// exists to end.
const HUNG: &str = "schema_version: 1
name: hung
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000d2
    type_ref: delta/pairer
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000d2
    to_port: a";

/// A scalar source beside a union output whose second emission is a
/// custom type: the port's scalar text is held first, then the custom
/// emission displaces it. Neither is wired downstream.
const SOURCES: &str = "schema_version: 1
name: sources
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000d3
    type_ref: delta/shaper
edges: []";

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

fn run_events(pushes: &[Value]) -> Vec<&Value> {
    pushes
        .iter()
        .filter(|push| push["type"] == "run_event")
        .collect()
}

fn statuses(pushes: &[Value]) -> Vec<(String, String)> {
    pushes
        .iter()
        .filter(|push| push["type"] == "node_status")
        .map(|push| {
            (
                push["node"].as_str().unwrap().to_owned(),
                push["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The run state the editor holds, waited for on the session itself: the
/// run's truth lives here even while every connection is stalled.
async fn wait_idle(editor: &Editor) -> Value {
    for _ in 0..500 {
        let run = send(editor, r#"{"id": 0, "type": "get_definition"}"#)["run"].clone();
        if run["running"] == json!(false) {
            return run;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the run never ended");
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
async fn every_event_is_forwarded_to_every_connection_as_it_occurs() {
    let editor = editor_holding(COUNTER_ALONE);
    let mut first = editor.subscribe();
    let mut second = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let (first_run, second_run) =
        tokio::join!(through_run_end(&mut first), through_run_end(&mut second));
    assert_eq!(
        first_run, second_run,
        "every connection rides the same stream"
    );

    let events = run_events(&first_run);
    let names: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().unwrap())
        .collect();
    assert_eq!(names.first(), Some(&"run_started"));
    assert_eq!(names.last(), Some(&"run_finished"));
    assert_eq!(
        names.iter().filter(|name| **name == "emitted").count(),
        50,
        "every emission crosses, none sampled away: {names:?}"
    );
    let started = names
        .iter()
        .position(|name| *name == "node_started")
        .unwrap();
    let first_emitted = names.iter().position(|name| *name == "emitted").unwrap();
    assert!(started < first_emitted, "a node runs before it emits");
    let completed = names
        .iter()
        .position(|name| *name == "node_completed")
        .unwrap();
    let last_emitted = names.iter().rposition(|name| *name == "emitted").unwrap();
    assert!(
        last_emitted < completed,
        "an emission precedes the completion"
    );
    let finished = events.last().unwrap();
    assert_eq!(finished["outcome"], json!("completed"));

    let emission = events
        .iter()
        .find(|event| event["event"] == "emitted")
        .unwrap();
    assert_eq!(
        emission["node"],
        json!("00000000-0000-0000-0000-0000000000d1")
    );
    assert_eq!(emission["port"], json!("out"));
    assert_eq!(
        emission["value"],
        json!("1"),
        "the scalar's plain text rides the event"
    );
}

#[tokio::test]
async fn the_resync_carries_the_run_state_and_the_per_node_display() {
    let editor = editor_holding(COUNTER_ALONE);
    let mut watcher = editor.subscribe();

    assert_eq!(
        held_display(&editor).await,
        json!({ "type": "run_display", "statuses": {}, "values": {} }),
        "before the session's first run the display is empty"
    );

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    through_run_end(&mut watcher).await;

    let display = held_display(&editor).await;
    assert_eq!(
        display["statuses"],
        json!({ "00000000-0000-0000-0000-0000000000d1": "completed" }),
        "the last run's status persists after the run ends"
    );
    assert_eq!(
        display["values"],
        json!({ "00000000-0000-0000-0000-0000000000d1/out": "50" }),
        "the counter's last ticked value is evidence of what the run did"
    );
    assert_eq!(
        held_display(&editor).await["values"],
        display["values"],
        "the display persists for as long as the run's does"
    );
}

#[tokio::test]
async fn scalar_emissions_carry_their_text_and_custom_emissions_carry_no_content() {
    let editor = editor_holding(SOURCES);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let pushes = through_run_end(&mut watcher).await;

    let scalar = run_events(&pushes)
        .into_iter()
        .find(|event| {
            event["event"] == "emitted"
                && event["node"] == json!("00000000-0000-0000-0000-0000000000d1")
        })
        .unwrap();
    assert_eq!(
        scalar["value"],
        json!("1"),
        "the base scalar renders its text"
    );

    let mut shaper = run_events(&pushes).into_iter().filter(|event| {
        event["event"] == "emitted"
            && event["node"] == json!("00000000-0000-0000-0000-0000000000d3")
    });
    let held = shaper.next().expect("the scalar emission crosses");
    assert_eq!(
        held["value"],
        json!("1"),
        "the union port's scalar emission carries its text like any other"
    );
    let custom = shaper
        .next()
        .expect("the custom-typed emission crosses — the wire animates from it");
    assert_eq!(custom["port"], json!("mark"));
    assert!(
        custom.get("value").is_none(),
        "nothing core would have to invent: {custom}"
    );

    let display = held_display(&editor).await;
    assert_eq!(
        display["statuses"],
        json!({
            "00000000-0000-0000-0000-0000000000d1": "completed",
            "00000000-0000-0000-0000-0000000000d3": "completed",
        })
    );
    assert_eq!(
        display["values"],
        json!({ "00000000-0000-0000-0000-0000000000d1/out": "50" }),
        "the custom emission is the port's latest, and it is nothing at all: the scalar it displaced no longer stands for the port"
    );
}

#[tokio::test]
async fn the_next_start_resets_the_canvas() {
    let editor = editor_holding(FAILING);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    through_run_end(&mut watcher).await;

    // Idle editing returns before the second run: the pairer, marked
    // stopped by the first run's failure, goes from the definition.
    send(
        &editor,
        r#"{"id": 2, "type": "delete_node", "uuid": "00000000-0000-0000-0000-0000000000d2"}"#,
    );
    send(&editor, r#"{"id": 3, "type": "start_run"}"#);
    through_run_end(&mut watcher).await;

    let ended = held_display(&editor).await["statuses"].clone();
    let mut named: Vec<String> = ended
        .as_object()
        .expect("statuses is an object")
        .keys()
        .cloned()
        .collect();
    named.sort();
    assert_eq!(
        named,
        vec![
            "00000000-0000-0000-0000-0000000000d1".to_owned(),
            "00000000-0000-0000-0000-0000000000d5".to_owned(),
        ],
        "the ended snapshot names exactly the second run's nodes: the deleted node's stale mark did not ride it"
    );
    assert_eq!(
        ended["00000000-0000-0000-0000-0000000000d5"],
        json!("failed"),
        "the failure's mark stands"
    );
}

#[tokio::test]
async fn a_failed_run_closes_started_nodes_as_stopped_and_names_the_failed_one() {
    let editor = editor_holding(FAILING);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let pushes = through_run_end(&mut watcher).await;

    let derived = statuses(&pushes);
    assert!(derived.contains(&(
        "00000000-0000-0000-0000-0000000000d5".to_owned(),
        "failed".to_owned()
    )));
    assert!(
        derived.contains(&(
            "00000000-0000-0000-0000-0000000000d2".to_owned(),
            "stopped".to_owned()
        )),
        "the started node the failure abandons closes as stopped: {derived:?}"
    );
    let failure = run_events(&pushes)
        .into_iter()
        .find(|event| event["event"] == "node_failed")
        .unwrap();
    assert_eq!(
        failure["node"],
        json!("00000000-0000-0000-0000-0000000000d5")
    );
    assert!(
        failure["error"]
            .as_str()
            .unwrap()
            .contains("the failer ran"),
        "the failure's explanation rides the forwarded event: {failure}"
    );

    let display = held_display(&editor).await;
    assert_eq!(
        display["statuses"]["00000000-0000-0000-0000-0000000000d5"],
        json!("failed")
    );
    assert_eq!(
        display["statuses"]["00000000-0000-0000-0000-0000000000d2"],
        json!("stopped"),
        "both marks stay findable after idle returns"
    );
}

#[tokio::test]
async fn a_user_stopped_run_closes_its_running_nodes_as_stopped() {
    let editor = editor_holding(HUNG);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let mut running = std::collections::HashSet::new();
    while running.len() < 2 {
        let push: Value = serde_json::from_str(&watcher.recv().await.expect("a push arrives"))
            .expect("a push is JSON");
        if push["type"] == "node_status" && push["status"] == json!("running") {
            running.insert(push["node"].as_str().unwrap().to_owned());
        }
    }
    assert_eq!(
        held_display(&editor).await["statuses"],
        json!({
            "00000000-0000-0000-0000-0000000000d1": "running",
            "00000000-0000-0000-0000-0000000000d2": "running",
        }),
        "a tab joining mid-run sees the run's own statuses"
    );

    send(&editor, r#"{"id": 2, "type": "stop_run"}"#);
    let pushes = through_run_end(&mut watcher).await;
    let mut closed: Vec<(String, String)> = statuses(&pushes);
    closed.sort();
    assert_eq!(
        closed,
        vec![
            (
                "00000000-0000-0000-0000-0000000000d1".to_owned(),
                "stopped".to_owned()
            ),
            (
                "00000000-0000-0000-0000-0000000000d2".to_owned(),
                "stopped".to_owned()
            ),
        ],
        "the stop closes every started node without its own final transition"
    );
    assert_eq!(
        held_display(&editor).await["statuses"],
        json!({
            "00000000-0000-0000-0000-0000000000d1": "stopped",
            "00000000-0000-0000-0000-0000000000d2": "stopped",
        })
    );
}

#[tokio::test]
async fn a_stalled_or_closed_connection_never_touches_the_run() {
    let editor = editor_holding(PAIRED);
    // One subscriber that never reads from here on — the stall — and one
    // that is closed outright.
    let _stalled = editor.subscribe();
    drop(editor.subscribe());

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let run = wait_idle(&editor).await;
    assert_eq!(
        run["outcome"],
        json!("completed"),
        "a stalled and a closed connection changed neither the run's completion nor its values"
    );
    assert_eq!(
        held_display(&editor).await["values"],
        json!({
            "00000000-0000-0000-0000-0000000000d1/out": "50",
            "00000000-0000-0000-0000-0000000000d2/sum": "51",
        }),
        "the run's own values, timing, and completion stand"
    );
}

#[tokio::test]
async fn a_connection_that_cannot_keep_up_is_dropped_over_its_socket_and_the_run_is_untouched() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let editor = Arc::new(editor_holding(&fan_out(1250)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(Arc::clone(&editor).serve(listener));
    let mut watcher = editor.subscribe();

    // The connection asks for a receive buffer a fraction of the run's
    // pushes hold, then stops reading: the pump's writes have nowhere to
    // go, and the push channel passes its bound while the pump sits
    // stalled mid-write.
    let socket = tokio::net::TcpSocket::new_v4().unwrap();
    socket.set_recv_buffer_size(4096).unwrap();
    let mut stalled = socket.connect(address).await.unwrap();
    stalled
        .write_all(
            b"GET /ws HTTP/1.1\r\nHost: editor\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        )
        .await
        .unwrap();
    let mut head = [0u8; 1024];
    let read = stalled.read(&mut head).await.unwrap();
    assert!(
        String::from_utf8_lossy(&head[..read]).starts_with("HTTP/1.1 101"),
        "the upgrade answer arrives"
    );

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let pushes = through_run_end(&mut watcher).await;
    assert_eq!(
        pushes.last().unwrap()["outcome"],
        json!("completed"),
        "the run never waits on a connection that cannot keep up"
    );

    // The server has dropped the stalled connection: reading now drains
    // whatever crossed before the drop, then meets the end the drop made.
    let mut dropped = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        stalled.read_to_end(&mut dropped),
    )
    .await
    .expect("the server ended the connection")
    .expect("the read does not fail");
    assert!(!dropped.is_empty(), "pushes crossed before the drop");

    assert_eq!(
        held_display(&editor).await["values"]["00000000-0000-0000-0000-0000000000d1/out"],
        json!("50"),
        "the dropped connection left the run's own display standing"
    );

    server.abort();
}
