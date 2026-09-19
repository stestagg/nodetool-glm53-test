//! What a binary hosting the editor reaches for: the consumer its runs'
//! unconnected outputs deliver to — supplied by the host, attached by the
//! server to every output a run leaves unconnected as one more downstream
//! of the same fan-out, the engine's consumer mechanism carried through —
//! and the unsaved-changes read the process-exit guard judges against.

mod server_common;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use nodetool::behaviour::Receiver;
use nodetool::graph;
use nodetool::server::{Editor, UnconnectedConsumer, UnconnectedStream};
use nodetool::Value;
use serde_json::json;
use server_common::*;
use test_plugin_delta as _;
use test_plugin_gamma as _;

/// A source that completes by itself, its `out` unconnected: fifty values
/// with nowhere wired to go.
const COUNTER_ALONE: &str = "schema_version: 1
name: counter
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
edges: []";

/// The counter wired into the doubler: the counter's output connected,
/// the doubler's the one the graph leaves unconnected.
const COUNTER_TO_DOUBLER: &str = "schema_version: 1
name: counter into doubler
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000d6
    type_ref: gamma/doubler
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000d6
    to_port: value";

/// A consumer gathering the i32 values its stream delivers, so a test
/// reads what the run produced after it ends. One consumer attaches per
/// unconnected output, so the sink it feeds is cloned per stream.
fn collector(into: Arc<Mutex<Vec<i32>>>) -> UnconnectedConsumer {
    Arc::new(move |mut values: Receiver<Value>| -> UnconnectedStream {
        let into = Arc::clone(&into);
        Box::pin(async move {
            while let Some(value) = values.recv().await {
                if let Some(count) = value.get::<i32>() {
                    into.lock()
                        .expect("the collector lock is never poisoned")
                        .push(*count);
                }
            }
            Ok(())
        })
    })
}

/// The push that ends a run — idle with its outcome — skipping the
/// running push a start rides and the definition pushes an edit rides.
async fn next_ending(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> serde_json::Value {
    loop {
        let pushed: serde_json::Value =
            serde_json::from_str(&receiver.recv().await.expect("a push arrives"))
                .expect("a push is JSON");
        if pushed["type"] == "run" && pushed["running"] == json!(false) {
            return pushed;
        }
    }
}

#[tokio::test]
async fn the_host_consumer_receives_each_unconnected_outputs_values_in_arrival_order() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let editor =
        editor_holding(COUNTER_ALONE).consume_unconnected(collector(Arc::clone(&received)));
    let mut watcher = editor.subscribe();

    let started = send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(started["type"], "run_started");
    assert_eq!(
        next_ending(&mut watcher).await["outcome"],
        json!("completed")
    );

    let received = received
        .lock()
        .expect("the collector lock is never poisoned");
    let expected: Vec<i32> = (1..=50).collect();
    assert_eq!(*received, expected, "every value, in arrival order");
}

#[tokio::test]
async fn a_connected_outputs_values_reach_only_their_wired_downstream() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let editor =
        editor_holding(COUNTER_TO_DOUBLER).consume_unconnected(collector(Arc::clone(&received)));
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(
        next_ending(&mut watcher).await["outcome"],
        json!("completed")
    );

    // The counter's output is wired: its values ride the connection to the
    // doubler alone, and what the consumer gathers is the unconnected
    // output's — the doubler's doubled stream.
    let received = received
        .lock()
        .expect("the collector lock is never poisoned");
    let expected: Vec<i32> = (1..=50).map(|count| count * 2).collect();
    assert_eq!(*received, expected);
}

#[tokio::test]
async fn a_host_supplying_no_consumer_discards_the_unconnected_outputs() {
    let editor = editor_holding(COUNTER_ALONE);
    let mut watcher = editor.subscribe();

    let started = send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(started["type"], "run_started");
    assert_eq!(
        next_ending(&mut watcher).await["outcome"],
        json!("completed"),
        "the unconnected output's values are discarded, the run none the worse"
    );
}

#[test]
fn the_unsaved_changes_state_is_the_dirty_state_the_exit_guard_judges() {
    let file = written_graph("host-seed", COUNTER_ALONE);
    let editor = editor_seeded(&file);
    assert!(
        !editor.unsaved_changes(),
        "a launch seeding is clean: the first Ctrl-C quits"
    );

    let edited = edit_parameter(
        &editor,
        1,
        "00000000-0000-0000-0000-0000000000d1",
        "start",
        Some("3"),
    );
    assert_eq!(edited["type"], "parameter_set");
    assert!(
        editor.unsaved_changes(),
        "an edit is unsaved: the guard warns and stands down"
    );

    let saved = send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "save_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(saved["type"], "file_saved");
    assert!(!editor.unsaved_changes(), "a save clears the state");

    let _ = fs::remove_file(&file);
}

/// An editor seeded the way a launch on a file seeds it: the loader's
/// definition and the file's path.
fn editor_seeded(path: &Path) -> Editor {
    let text = fs::read_to_string(path).expect("the seed is readable");
    Editor::new(
        graph::load(&text).expect("the seed is loadable"),
        Some(path.to_string_lossy().into_owned()),
    )
}
