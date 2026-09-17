//! The editor server's run: start compiling the held definition afresh,
//! stop ending a run beside its natural ends, the outcomes — completed,
//! failed, stopped — each returning to idle, the editing lock a running
//! run holds, and the run state pushed to every connection and carried in
//! the connect-time resync.

mod server_common;

use std::fs;

use nodetool::server::Editor;
use serde_json::{json, Value};
use server_common::*;
use test_plugin_delta as _;
use test_plugin_gamma as _;

/// A source that completes by itself: the counter, alone.
const COUNTER_ALONE: &str = "schema_version: 1
name: counter
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
edges: []";

/// A run that ends on a node's error: the counter feeding the failer.
const FAILING: &str = "schema_version: 1
name: failing
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000d1
    type_ref: delta/counter
  - uuid: 00000000-0000-0000-0000-0000000000d5
    type_ref: delta/failer
edges:
  - from: 00000000-0000-0000-0000-0000000000d1
    from_port: out
    to: 00000000-0000-0000-0000-0000000000d5
    to_port: value";

/// A run that never ends by itself: the pairer's second input is
/// connected in shape but fed by nothing, so the gate never opens — the
/// hang a stop exists to end.
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

/// The next run-state push, skipping the definition pushes an edit rides.
async fn next_run(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> Value {
    loop {
        let pushed: Value = serde_json::from_str(&receiver.recv().await.expect("a push arrives"))
            .expect("a push is JSON");
        if pushed["type"] == "run" {
            return pushed;
        }
    }
}

/// The run state the editor holds, as the resync carries it.
fn held_run(editor: &Editor) -> Value {
    send(editor, r#"{"id": 0, "type": "get_definition"}"#)["run"].clone()
}

#[tokio::test]
async fn the_run_state_rides_the_resync_and_is_pushed_to_every_connection() {
    let editor = editor_holding(COUNTER_ALONE);
    let mut first = editor.subscribe();
    let mut second = editor.subscribe();

    let resync = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);
    assert_eq!(
        resync["run"],
        json!({ "running": false, "outcome": null, "error": null }),
        "an editor that has not run carries no outcome"
    );

    let started = send(&editor, r#"{"id": 2, "type": "start_run"}"#);
    assert_eq!(started["type"], "run_started");
    for watcher in [&mut first, &mut second] {
        assert_eq!(
            next_run(watcher).await["running"],
            json!(true),
            "every connection is pushed the running state"
        );
    }

    // A connection arriving mid-run resyncs to the running state.
    let mut latecomer = editor.subscribe();
    assert_eq!(held_run(&editor)["running"], json!(true));

    for watcher in [&mut first, &mut second, &mut latecomer] {
        let ended = next_run(watcher).await;
        assert_eq!(ended["running"], json!(false));
        assert_eq!(
            ended["outcome"],
            json!("completed"),
            "the run ended by itself, the outcome pushed everywhere"
        );
    }
}

#[tokio::test]
async fn start_compiles_the_held_definition_afresh_so_an_edit_since_the_last_run_is_what_runs() {
    let editor = editor_holding(COUNTER_ALONE);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));
    assert_eq!(next_run(&mut watcher).await["outcome"], json!("completed"));

    // The edit lands between the runs: a failer wired to the counter's
    // output. The next start compiles the edited definition, and the run
    // ends on the failer's error.
    let failer = create(&editor, 2, "delta/failer");
    send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "00000000-0000-0000-0000-0000000000d1", "from_port": "out", "to": "{failer}", "to_port": "value"}}"#
        ),
    );

    send(&editor, r#"{"id": 4, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));
    let ended = next_run(&mut watcher).await;
    assert_eq!(ended["outcome"], json!("failed"));
    let failure = ended["error"].as_str().unwrap();
    assert!(
        failure.contains(&failer) && failure.contains("the failer ran"),
        "the failure names the node the edited definition runs: {failure}"
    );
}

#[tokio::test]
async fn a_compile_failure_is_answered_with_the_errors_and_no_run_starts() {
    let editor = editor_holding(COUNTER_ALONE);
    let doubler_1 = create(&editor, 1, "gamma/doubler");
    let doubler_2 = create(&editor, 2, "gamma/doubler");
    for (id, from, to) in [
        (
            3,
            "00000000-0000-0000-0000-0000000000d1",
            doubler_1.as_str(),
        ),
        (4, doubler_1.as_str(), doubler_2.as_str()),
        (5, doubler_2.as_str(), doubler_1.as_str()),
    ] {
        send(
            &editor,
            &format!(
                r#"{{"id": {id}, "type": "wire", "from": "{from}", "from_port": "out", "to": "{to}", "to_port": "value"}}"#
            ),
        );
    }

    // Subscribed only now, past the edits: nothing is pushed for a
    // refused start, neither the running state nor anything else.
    let mut watcher = editor.subscribe();
    let refused = send(&editor, r#"{"id": 6, "type": "start_run"}"#);
    assert_eq!(refused["type"], "error");
    let errors = refused["error"].as_str().unwrap();
    assert!(
        errors.contains("cycle")
            && errors.contains(doubler_1.as_str())
            && errors.contains(doubler_2.as_str()),
        "the compile errors name what and where: {errors}"
    );
    assert!(
        watcher.try_recv().is_err(),
        "no run state is pushed: the state stays idle"
    );
    assert_eq!(held_run(&editor)["running"], json!(false));

    // The editor never polices: editing is exactly as free as it was.
    let edited = create(&editor, 7, "delta/counter");
    assert_eq!(edited.len(), 36, "editing works after a refused start");
}

#[tokio::test]
async fn while_a_run_is_on_every_editing_operation_is_refused_and_the_definition_is_untouched() {
    let editor = editor_holding(HUNG);
    let before = held_definition(&editor);
    let file = written_graph("run-open", COUNTER_ALONE);
    let target = temp_path("run-save");
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));

    for message in [
        r#"{"id": 2, "type": "create_node", "type_ref": "delta/counter", "position": {"x": 0, "y": 0}}"#,
        r#"{"id": 3, "type": "move_node", "uuid": "00000000-0000-0000-0000-0000000000d1", "position": {"x": 9, "y": 9}}"#,
        r#"{"id": 4, "type": "set_label", "uuid": "00000000-0000-0000-0000-0000000000d1", "label": "renamed"}"#,
        r#"{"id": 5, "type": "set_parameter", "uuid": "00000000-0000-0000-0000-0000000000d2", "input": "b", "value": "3"}"#,
        r#"{"id": 6, "type": "wire", "from": "00000000-0000-0000-0000-0000000000d1", "from_port": "out", "to": "00000000-0000-0000-0000-0000000000d2", "to_port": "b"}"#,
        r#"{"id": 7, "type": "unhook", "to": "00000000-0000-0000-0000-0000000000d2", "to_port": "a"}"#,
        r#"{"id": 8, "type": "delete_node", "uuid": "00000000-0000-0000-0000-0000000000d1"}"#,
        &format!(
            r#"{{"id": 9, "type": "open_file", "path": {}}}"#,
            path_field(&file)
        ),
        &format!(
            r#"{{"id": 10, "type": "save_file", "path": {}}}"#,
            path_field(&target)
        ),
        r#"{"id": 11, "type": "new_graph"}"#,
    ] {
        let reply = send(&editor, message);
        assert_eq!(reply["type"], "error", "for {message}");
        assert!(
            reply["error"].as_str().unwrap().contains("a run is on"),
            "the refusal names the running state: {reply}"
        );
    }

    assert_eq!(
        held_definition(&editor),
        before,
        "the definition is untouched: every operation was refused"
    );
    assert!(!target.exists(), "the save never reached the disk");
    let _ = fs::remove_file(&file);

    // The stop ends the run, and editing is back with the stopped outcome.
    send(&editor, r#"{"id": 12, "type": "stop_run"}"#);
    let stopped = next_run(&mut watcher).await;
    assert_eq!(stopped["running"], json!(false));
    assert_eq!(stopped["outcome"], json!("stopped"));
    let edit = edit_label(
        &editor,
        13,
        "00000000-0000-0000-0000-0000000000d1",
        "renamed",
    );
    assert_eq!(
        edit["type"], "label_set",
        "editing re-enables with the idle state"
    );
    let _ = fs::remove_file(&target);
}

#[tokio::test]
async fn a_stopped_run_is_followed_by_a_clean_restart() {
    let editor = editor_holding(HUNG);
    let mut watcher = editor.subscribe();

    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));
    send(&editor, r#"{"id": 2, "type": "stop_run"}"#);
    assert_eq!(next_run(&mut watcher).await["outcome"], json!("stopped"));

    send(&editor, r#"{"id": 3, "type": "start_run"}"#);
    assert_eq!(
        next_run(&mut watcher).await["running"],
        json!(true),
        "the second start begins a run of its own"
    );
    send(&editor, r#"{"id": 4, "type": "stop_run"}"#);
    assert_eq!(next_run(&mut watcher).await["outcome"], json!("stopped"));
}

#[tokio::test]
async fn natural_completion_and_fail_fast_each_return_to_idle_with_their_outcomes() {
    let editor = editor_holding(COUNTER_ALONE);
    let mut watcher = editor.subscribe();
    send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));
    let completed = next_run(&mut watcher).await;
    assert_eq!(completed["running"], json!(false));
    assert_eq!(
        completed["outcome"],
        json!("completed"),
        "every node done ends the run by itself, nobody stopping it"
    );

    let editor = editor_holding(FAILING);
    let mut watcher = editor.subscribe();
    send(&editor, r#"{"id": 2, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));
    let failed = next_run(&mut watcher).await;
    assert_eq!(failed["running"], json!(false));
    assert_eq!(failed["outcome"], json!("failed"));
    let failure = failed["error"].as_str().unwrap();
    assert!(
        failure.contains("00000000-0000-0000-0000-0000000000d5")
            && failure.contains("the failer ran"),
        "the failure names the node instance and what failed: {failure}"
    );
    assert_eq!(held_run(&editor)["running"], json!(false));
}

#[tokio::test]
async fn start_while_running_stop_while_idle_and_start_on_an_empty_definition_are_refused() {
    let editor = editor();
    let mut watcher = editor.subscribe();

    let empty = send(&editor, r#"{"id": 1, "type": "start_run"}"#);
    assert_eq!(empty["type"], "error");
    assert!(
        empty["error"].as_str().unwrap().contains("no nodes"),
        "the refusal names the empty definition"
    );
    let idle = send(&editor, r#"{"id": 2, "type": "stop_run"}"#);
    assert_eq!(idle["type"], "error");
    assert!(
        idle["error"].as_str().unwrap().contains("no run"),
        "the refusal names the idle state"
    );
    let usable = send(&editor, r#"{"id": 3, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 3, "the connection is still usable");

    create(&editor, 4, "delta/counter");
    send(&editor, r#"{"id": 5, "type": "start_run"}"#);
    assert_eq!(next_run(&mut watcher).await["running"], json!(true));

    let again = send(&editor, r#"{"id": 6, "type": "start_run"}"#);
    assert_eq!(again["type"], "error");
    assert!(
        again["error"].as_str().unwrap().contains("already on"),
        "the refusal names the running state"
    );
    assert_eq!(held_run(&editor)["running"], json!(true));

    send(&editor, r#"{"id": 7, "type": "stop_run"}"#);
    assert_eq!(next_run(&mut watcher).await["outcome"], json!("stopped"));
    assert_eq!(held_run(&editor)["running"], json!(false));
}
