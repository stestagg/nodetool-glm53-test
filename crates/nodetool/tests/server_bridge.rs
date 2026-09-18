//! The bridge: the run's events forwarded to every connection as they
//! occur, the node statuses derived once server-side from those events —
//! story 07's closure rule, extended to the stopped outcome — the latest
//! value per emitting port held and riding the connect-time resync, and
//! runs that never wait on a connection: a stalled or closed one leaves
//! the run's values, timing, and completion untouched, and one whose
//! outbound queue passes its bound is dropped.

mod server_common;

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

/// A scalar source beside a custom-typed one, neither wired downstream.
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

fn held_display(editor: &Editor) -> Value {
    send(editor, r#"{"id": 0, "type": "get_definition"}"#)["run_display"].clone()
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
        held_display(&editor),
        json!({ "statuses": {}, "values": [] }),
        "before the session's first run the display is empty"
    );

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    through_run_end(&mut watcher).await;

    let display = held_display(&editor);
    assert_eq!(
        display["statuses"],
        json!({ "00000000-0000-0000-0000-0000000000d1": "completed" }),
        "the last run's status persists after the run ends"
    );
    assert_eq!(
        display["values"],
        json!([{
            "node": "00000000-0000-0000-0000-0000000000d1",
            "port": "out",
            "value": "50",
        }]),
        "the counter's last ticked value is evidence of what the run did"
    );
    assert_eq!(
        held_display(&editor)["values"],
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

    let emissions: Vec<&Value> = run_events(&pushes)
        .into_iter()
        .filter(|event| event["event"] == "emitted")
        .collect();
    let scalar = emissions
        .iter()
        .find(|event| event["node"] == json!("00000000-0000-0000-0000-0000000000d1"))
        .unwrap();
    assert_eq!(
        scalar["value"],
        json!("1"),
        "the base scalar renders its text"
    );
    let custom = emissions
        .iter()
        .find(|event| event["node"] == json!("00000000-0000-0000-0000-0000000000d3"))
        .expect("the custom-typed emission crosses — the wire animates from it");
    assert_eq!(custom["port"], json!("mark"));
    assert!(
        custom.get("value").is_none(),
        "nothing core would have to invent: {custom}"
    );

    let display = held_display(&editor);
    assert_eq!(
        display["statuses"],
        json!({
            "00000000-0000-0000-0000-0000000000d1": "completed",
            "00000000-0000-0000-0000-0000000000d3": "completed",
        })
    );
    assert_eq!(
        display["values"],
        json!([{
            "node": "00000000-0000-0000-0000-0000000000d1",
            "port": "out",
            "value": "50",
        }]),
        "only what core can render is held for the snapshot"
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

    let display = held_display(&editor);
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
        held_display(&editor)["statuses"],
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
        held_display(&editor)["statuses"],
        json!({
            "00000000-0000-0000-0000-0000000000d1": "stopped",
            "00000000-0000-0000-0000-0000000000d2": "stopped",
        })
    );
}

#[tokio::test]
async fn a_stalled_or_closed_connection_never_touches_the_run_and_one_past_its_bound_is_dropped() {
    let editor = editor_holding(PAIRED);
    let mut stalled = editor.subscribe();
    let closed = editor.subscribe();
    drop(closed);

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    let run = wait_idle(&editor).await;
    assert_eq!(
        run["outcome"],
        json!("completed"),
        "a stalled and a closed connection changed neither the run's completion nor its values"
    );
    assert_eq!(
        held_display(&editor)["values"],
        json!([
            {
                "node": "00000000-0000-0000-0000-0000000000d1",
                "port": "out",
                "value": "50",
            },
            {
                "node": "00000000-0000-0000-0000-0000000000d2",
                "port": "sum",
                "value": "51",
            },
        ]),
        "the run's own values, timing, and completion stand"
    );

    match stalled.try_recv() {
        Err(broadcast::error::TryRecvError::Lagged(_)) => {}
        other => panic!(
            "a connection whose outbound queue passed its bound is dropped, not served stale: {other:?}"
        ),
    }
}
