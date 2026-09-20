//! The problems state the server holds beside the definition: recomputed
//! through the same compile a start runs after every definition change and
//! when a file is opened, pushed whole to every connection, carried in the
//! connect-time resync, and cleared when the fix lands. The marks advise:
//! a graph with problems edits, saves, and starts exactly as freely as a
//! clean one.

mod server_common;

use std::fs;

use nodetool::server::Editor;
use serde_json::{json, Value};
use server_common::*;
use test_plugin_alpha as _;
use test_plugin_beta as _;
use test_plugin_gamma as _;

/// A definition carrying one compile problem of each kind: the adder's
/// `a` parameterised while `b` is neither connected nor parameterised —
/// the hang gate, a warning — and the identity's `value` holding a string
/// literal nothing bridges, an error.
const PROBLEMATIC: &str = "schema_version: 1
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: alpha/add
    parameters:
      a: 1
  - uuid: 0000b100-0000-0000-0000-0000000000b1
    type_ref: beta/identity
    parameters:
      value: three
edges: []";

/// The problems a reply or push carries, as the protocol states them.
fn problems(reply: &Value) -> &Value {
    reply
        .get("problems")
        .expect("the problems travel beside the definition")
}

/// The problems the editor holds, as the connect-time resync carries them.
fn held_problems(editor: &Editor) -> Value {
    problems(&send(editor, r#"{"id": 0, "type": "get_definition"}"#)).clone()
}

/// The definition push, skipping any other message a watcher queued.
fn push_definition(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> Value {
    loop {
        let pushed: Value = serde_json::from_str(
            &receiver
                .try_recv()
                .expect("the definition push arrives with the edit"),
        )
        .expect("a push is JSON");
        if pushed["type"] == "definition" {
            return pushed;
        }
    }
}

#[test]
fn the_launch_seed_carries_the_problems_in_the_connect_time_resync() {
    let editor = editor_holding(PROBLEMATIC);

    let held = held_problems(&editor);
    let listed = held.as_array().unwrap();
    assert_eq!(listed.len(), 2, "one error and one warning: {listed:?}");

    let error = listed
        .iter()
        .find(|problem| problem["message"].as_str().unwrap().contains("literal"))
        .expect("the unresolvable literal is an error");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("0000b100-0000-0000-0000-0000000000b1"),
        "{error}"
    );
    assert_eq!(
        error["nodes"],
        json!(["0000b100-0000-0000-0000-0000000000b1"]),
        "the attribution rides the structure, not the message: {error}"
    );

    let warning = listed
        .iter()
        .find(|problem| problem["message"].as_str().unwrap().contains("hang"))
        .expect("the starving input beside a parameterised one is warned");
    assert!(
        warning["message"].as_str().unwrap().contains("`b`"),
        "the warning names the input: {warning}"
    );
    assert_eq!(
        warning["nodes"],
        json!(["00000000-0000-0000-0000-0000000000a1"]),
        "{warning}"
    );
}

#[test]
fn an_edit_that_introduces_a_problem_pushes_it_to_every_connection_and_a_fix_clears_it() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let identity = create(&editor, 2, "beta/identity");
    let sink = create(&editor, 3, "gamma/string_sink");
    let mut watchers = [editor.subscribe(), editor.subscribe()];

    // `a` parameterised, `b` starving beside it: the hang gate warns,
    // naming the node and the input.
    let edited = edit_parameter(&editor, 1, &adder, "a", Some("1"));
    assert_eq!(edited["type"], "parameter_set");
    for watcher in &mut watchers {
        let pushed = push_definition(watcher);
        let listed = pushed["problems"].as_array().unwrap();
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert!(
            listed[0]["message"].as_str().unwrap().contains("`b`")
                && listed[0]["nodes"] == json!([adder]),
            "{listed:?}"
        );
    }

    // A wire that resolves changes nothing: the warning stands.
    send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "wire", "from": "{adder}", "from_port": "sum", "to": "{identity}", "to_port": "value"}}"#
        ),
    );
    let pushed = push_definition(&mut watchers[0]);
    let listed = pushed["problems"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");

    // A wire nothing bridges adds its error, naming both ends.
    send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "{adder}", "from_port": "sum", "to": "{sink}", "to_port": "text"}}"#
        ),
    );
    let pushed = push_definition(&mut watchers[0]);
    let listed = pushed["problems"].as_array().unwrap();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(
        listed.iter().any(|problem| problem["message"]
            .as_str()
            .unwrap()
            .contains("no exact match and no declared conversion")
            && problem["nodes"] == json!([adder, sink])),
        "{listed:?}"
    );

    // Unhooking the impossible wire clears its error on the next
    // recompute.
    send(
        &editor,
        &format!(r#"{{"id": 4, "type": "unhook", "to": "{sink}", "to_port": "text"}}"#),
    );
    let pushed = push_definition(&mut watchers[0]);
    let listed = pushed["problems"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");

    // Feeding `b` leaves nothing starving: a clean graph carries nothing,
    // on the push and in the resync alike.
    let edited = edit_parameter(&editor, 5, &adder, "b", Some("2"));
    assert_eq!(edited["type"], "parameter_set");
    let pushed = push_definition(&mut watchers[0]);
    assert!(
        pushed["problems"].as_array().unwrap().is_empty(),
        "the quiet canvas is the normal: {pushed}"
    );
    assert_eq!(held_problems(&editor), json!([]));
}

#[test]
fn a_fresh_node_s_unset_choice_is_a_problem_the_pick_clears() {
    // A node dropped from the palette has made no choice yet, so its
    // state is a compile error the canvas marks at once — never a
    // surprise at the next start. The select's commit is an ordinary
    // parameter edit, and the recompute beside it clears the mark.
    let editor = editor();
    let mut watcher = editor.subscribe();
    let dial = create(&editor, 1, "beta/dial");

    let pushed = push_definition(&mut watcher);
    let listed = pushed["problems"].as_array().unwrap();
    let unset = listed
        .iter()
        .find(|problem| problem["message"].as_str().unwrap().contains("`mode`"))
        .expect("the unset choice is a problem");
    assert!(
        unset["message"].as_str().unwrap().contains("up, down"),
        "the problem names the options: {unset}"
    );
    assert_eq!(unset["nodes"], json!([dial]), "{unset}");

    let edited = edit_parameter(&editor, 2, &dial, "mode", Some("down"));
    assert_eq!(edited["type"], "parameter_set");
    let pushed = push_definition(&mut watcher);
    assert!(
        !pushed["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|problem| problem["message"].as_str().unwrap().contains("`mode`")),
        "the pick cleared the mark: {pushed}"
    );
}

#[test]
fn an_open_recomputes_the_problems_the_opened_definition_carries() {
    let editor = editor();
    let file = written_graph("problems-open", PROBLEMATIC);

    let opened = send(
        &editor,
        &format!(
            r#"{{"id": 1, "type": "open_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(opened["type"], "file_opened");
    let held = held_problems(&editor);
    assert_eq!(held.as_array().unwrap().len(), 2, "{held:?}");

    // Opening a clean file leaves the problems clean.
    let clean = written_graph(
        "problems-open-clean",
        "schema_version: 1
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: alpha/add
edges: []",
    );
    send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "open_file", "path": {}}}"#,
            path_field(&clean)
        ),
    );
    assert_eq!(
        held_problems(&editor),
        json!([]),
        "the adder's inputs both starve: nothing arrives, nothing warns"
    );

    let _ = fs::remove_file(&file);
    let _ = fs::remove_file(&clean);
}

#[tokio::test]
async fn a_graph_with_problems_still_edits_saves_and_starts_as_freely_as_a_clean_one() {
    let editor = editor_holding(PROBLEMATIC);

    // Editing works.
    let moved = send(
        &editor,
        r#"{"id": 1, "type": "move_node", "uuid": "00000000-0000-0000-0000-0000000000a1", "position": {"x": 9, "y": 9}}"#,
    );
    assert_eq!(moved["type"], "node_moved");

    // Saving works.
    let file = temp_path("problems-save");
    let saved = send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "save_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(saved["type"], "file_saved");

    // Starting is compile's to refuse, with the errors — never the
    // editor's, and never for the warnings.
    let refused = send(&editor, r#"{"id": 3, "type": "start_run"}"#);
    assert_eq!(refused["type"], "error");
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("literal \"three\""),
        "the failed start names the compile error: {refused}"
    );

    let warned_only = "schema_version: 1
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: alpha/add
    parameters:
      a: 1
edges: []";
    let editor = editor_holding(warned_only);
    let started = send(&editor, r#"{"id": 4, "type": "start_run"}"#);
    assert_eq!(
        started["type"], "run_started",
        "the hang-gate warning blocks nothing: {started}"
    );
    send(&editor, r#"{"id": 5, "type": "stop_run"}"#);

    let _ = fs::remove_file(&file);
}

#[test]
fn an_unknown_typed_node_carries_its_unknown_type_error() {
    let editor = editor_holding(
        "schema_version: 1
nodes:
  - uuid: 0000c100-0000-0000-0000-0000000000c1
    type_ref: nobody/nothing
edges: []",
    );

    let held = held_problems(&editor);
    let listed = held.as_array().unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        listed[0]["message"]
            .as_str()
            .unwrap()
            .contains("no linked plugin declares"),
        "the placeholder's inertness explained: {listed:?}"
    );
    assert_eq!(
        listed[0]["nodes"],
        json!(["0000c100-0000-0000-0000-0000000000c1"])
    );
}

#[test]
fn a_fresh_graph_carries_no_problems() {
    let editor = editor();
    create(&editor, 1, "alpha/add");
    create(&editor, 2, "beta/identity");
    assert_eq!(
        held_problems(&editor),
        json!([]),
        "every input of each node starves together: nothing arrives, so nothing warns"
    );
}
