//! What a binary hosting the editor reaches for: the taps its runs deliver
//! through — the host naming a node type and an input port of its own
//! plugin, the server attaching a consumer to every instance the graph the
//! run compiles carries, the engine's tap mechanism carried through — and
//! the unsaved-changes read the process-exit guard judges against.

mod server_common;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use nodetool::behaviour::Receiver;
use nodetool::graph;
use nodetool::server::{Editor, TapConsumer, TapStream};
use nodetool::Value;
use serde_json::json;
use server_common::*;
use test_plugin_delta as _;
use test_plugin_gamma as _;

/// The node type the host taps in these tests and the input it listens on:
/// a fixture terminus, standing for a hosting plugin's output node.
const SINK: &str = "delta/terminus";
const SINK_INPUT: &str = "value";

/// A source that completes by itself, its fifty values wired into a
/// terminus.
const COUNTER_TO_SINK: &str = "schema_version: 1
name: counter into sink
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: delta/terminus
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000a1
    to_port: value";

/// The counter wired into a doubler and nowhere else: a graph carrying no
/// instance of the tapped type, its doubled values leaving by no port.
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

/// The counter feeding two termini: one tap per instance, each its own
/// stream of the same fifty values.
const COUNTER_TO_TWO_SINKS: &str = "schema_version: 1
name: counter into two sinks
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: delta/terminus
  - uuid: 00000000-0000-0000-0000-0000000000a2
    type_ref: delta/terminus
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000a1
    to_port: value
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000a2
    to_port: value";

/// A terminus nothing wires, its input carrying a parameter literal alone.
const SINK_ON_A_LITERAL: &str = "schema_version: 1
name: sink on a literal
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: delta/terminus
    parameters:
      value: 7
edges: []";

/// A consumer gathering the i32 values its stream delivers, so a test
/// reads what the run produced after it ends. One consumer attaches per
/// tapped instance, so the vec it feeds is cloned per stream.
fn collector(into: Arc<Mutex<Vec<i32>>>) -> TapConsumer {
    Arc::new(move |mut values: Receiver<Value>| -> TapStream {
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

/// An editor over `text`, tapping the terminus type into a vec the test
/// reads.
fn tapped(text: &str) -> (Editor, Arc<Mutex<Vec<i32>>>) {
    let received = Arc::new(Mutex::new(Vec::new()));
    let editor = editor_holding(text).tap(SINK, SINK_INPUT, collector(Arc::clone(&received)));
    (editor, received)
}

/// Start the run and wait for its ending, answering the outcome's name.
async fn ran(editor: &Editor) -> serde_json::Value {
    let mut watcher = editor.subscribe();
    let started = send(editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(started["type"], "run_started");
    loop {
        let pushed: serde_json::Value =
            serde_json::from_str(&watcher.recv().await.expect("a push arrives"))
                .expect("a push is JSON");
        if pushed["type"] == "run" && pushed["running"] == json!(false) {
            return pushed["outcome"].clone();
        }
    }
}

/// What the collector gathered, once the run is over.
fn gathered(received: &Arc<Mutex<Vec<i32>>>) -> Vec<i32> {
    received
        .lock()
        .expect("the collector lock is never poisoned")
        .clone()
}

#[tokio::test]
async fn the_host_tap_receives_the_tapped_inputs_values_in_arrival_order() {
    let (editor, received) = tapped(COUNTER_TO_SINK);

    assert_eq!(ran(&editor).await, json!("completed"));

    let expected: Vec<i32> = (1..=50).collect();
    assert_eq!(
        gathered(&received),
        expected,
        "every value, in arrival order"
    );
}

#[tokio::test]
async fn a_graph_carrying_no_instance_of_the_tapped_type_delivers_nothing() {
    // The doubler's output goes nowhere, and that is no longer anybody's
    // business: the tap is keyed to the input the host named, not to what
    // the graph leaves unwired.
    let (editor, received) = tapped(COUNTER_TO_DOUBLER);

    assert_eq!(ran(&editor).await, json!("completed"));

    assert!(gathered(&received).is_empty(), "a silent, complete run");
}

#[tokio::test]
async fn every_instance_of_the_tapped_type_is_tapped_on_its_own_stream() {
    let (editor, received) = tapped(COUNTER_TO_TWO_SINKS);

    assert_eq!(ran(&editor).await, json!("completed"));

    let mut gathered = gathered(&received);
    gathered.sort_unstable();
    let expected: Vec<i32> = (1..=50).flat_map(|count| [count, count]).collect();
    assert_eq!(gathered, expected, "each sink saw the whole stream");
}

#[tokio::test]
async fn a_parameter_literal_on_a_tapped_input_is_a_value_it_receives() {
    let (editor, received) = tapped(SINK_ON_A_LITERAL);

    assert_eq!(ran(&editor).await, json!("completed"));

    assert_eq!(gathered(&received), [7], "the literal arrives once");
}

#[tokio::test]
async fn a_host_naming_no_tap_leaves_the_run_untapped_and_none_the_worse() {
    let editor = editor_holding(COUNTER_TO_SINK);

    assert_eq!(ran(&editor).await, json!("completed"));
}

#[tokio::test]
async fn the_taps_follow_the_graph_the_next_run_compiles() {
    // The host named its interest by type, so a terminus wired in the
    // browser is tapped by the very next run, and one deleted there stops.
    let received = Arc::new(Mutex::new(Vec::new()));
    let editor = editor_holding("schema_version: 1\nname: empty\nnodes: []\nedges: []").tap(
        SINK,
        SINK_INPUT,
        collector(Arc::clone(&received)),
    );
    let counter = create(&editor, 10, "delta/counter");
    let sink = create(&editor, 11, SINK);
    let wired = send(
        &editor,
        &format!(
            r#"{{"id": 12, "type": "wire", "from": "{counter}", "from_port": "out", "to": "{sink}", "to_port": "value"}}"#
        ),
    );
    assert_eq!(wired["type"], "wired");

    assert_eq!(ran(&editor).await, json!("completed"));
    assert_eq!(
        gathered(&received).len(),
        50,
        "the terminus added here is tapped"
    );

    let deleted = send(
        &editor,
        &format!(r#"{{"id": 13, "type": "delete_node", "uuid": "{sink}"}}"#),
    );
    assert_eq!(deleted["type"], "node_deleted");

    assert_eq!(ran(&editor).await, json!("completed"));
    assert_eq!(
        gathered(&received).len(),
        50,
        "the terminus deleted here stops: the second run delivered nothing"
    );
}

#[test]
fn the_unsaved_changes_state_is_the_dirty_state_the_exit_guard_judges() {
    let file = written_graph("host-seed", COUNTER_TO_SINK);
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
