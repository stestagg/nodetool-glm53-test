//! The editor server's operations against the held session: the listing,
//! the create, move, label, parameter, wire, unhook, and delete
//! operations, the file operations — launch-with-file, open, save, fresh
//! graph — and every malformed operation path. The run — start, stop, the
//! outcomes, the editing lock, and the run state's pushes and resync — is
//! server_run.rs's. The transport — the greeting, the envelope, the
//! served socket — is connection.rs's.

mod server_common;

use std::fs;

use nodetool::graph;
use nodetool::registry;
use nodetool::server::Editor;
use serde_json::{json, Value};
use server_common::*;
use test_plugin_alpha as _;
use test_plugin_beta as _;

fn push(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> Value {
    serde_json::from_str(&receiver.blocking_recv().expect("a push arrives"))
        .expect("a push is JSON")
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
fn a_label_edit_stores_the_override_and_pushes_it_to_every_connection() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let mut watchers = [editor.subscribe(), editor.subscribe()];

    let renamed = edit_label(&editor, 2, &adder, "The adder");
    assert_eq!(renamed["type"], "label_set");
    assert_eq!(renamed["id"], 2);

    let definition = held_definition(&editor);
    assert_eq!(definition["nodes"][0]["label"], "The adder");

    for watcher in &mut watchers {
        let pushed = push(watcher);
        assert_eq!(pushed["type"], "definition");
        assert_eq!(pushed["graph"]["nodes"][0]["label"], "The adder");
    }
}

#[test]
fn clearing_the_label_override_returns_the_node_to_its_default_label() {
    let editor = editor_holding(SEEDED);
    let adder = "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33";

    let cleared = edit_label(&editor, 1, adder, "");
    assert_eq!(cleared["type"], "label_set");
    assert_eq!(cleared["id"], 1);
    let node = &held_definition(&editor)["nodes"][0];
    assert!(
        node.get("label").is_none(),
        "an empty label means the type's default, no empty-string stand-in: {node}"
    );

    // And a fresh override lands again after a clear.
    edit_label(&editor, 2, adder, "Renamed");
    assert_eq!(held_definition(&editor)["nodes"][0]["label"], "Renamed");
}

#[test]
fn label_and_parameter_edits_naming_an_unknown_node_are_errors() {
    let editor = editor_holding(SEEDED);
    let before = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);
    let unknown = "0d5c1e2a-3b4c-4d5e-8f90-1a2b3c4d5e6f";

    let renamed = edit_label(&editor, 2, unknown, "Renamed");
    assert_eq!(renamed["type"], "error");
    assert!(renamed["error"].as_str().unwrap().contains("no node"));

    let edited = edit_parameter(&editor, 3, unknown, "a", Some("1"));
    assert_eq!(edited["type"], "error");
    assert!(edited["error"].as_str().unwrap().contains("no node"));

    let after = send(&editor, r#"{"id": 4, "type": "get_definition"}"#);
    assert_eq!(
        before["graph"], after["graph"],
        "the definition is untouched"
    );
    assert_eq!(after["id"], 4, "the connection is still usable");
}

#[test]
fn a_parameter_edit_stores_the_scalar_with_the_file_formats_kind() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let identity = create(&editor, 2, "beta/identity");

    // `2.5` on the identity's `value`, declared [i32, f64]: stored a
    // float — the declared union is not policed at edit time.
    let edited = edit_parameter(&editor, 3, &identity, "value", Some("2.5"));
    assert_eq!(edited["type"], "parameter_set");
    let value = &held_definition(&editor)["nodes"][1]["parameters"]["value"];
    assert_eq!(value.as_f64(), Some(2.5), "a committed 2.5 is a float");
    assert_eq!(value.as_i64(), None, "not an integer");

    // `true` on the adder's `a`, though it declares i32: stored a boolean.
    edit_parameter(&editor, 4, &adder, "a", Some("true"));
    let value = &held_definition(&editor)["nodes"][0]["parameters"]["a"];
    assert_eq!(value.as_bool(), Some(true));

    // `7` on the same input: an integer, replacing the boolean.
    edit_parameter(&editor, 5, &adder, "a", Some("7"));
    let value = &held_definition(&editor)["nodes"][0]["parameters"]["a"];
    assert_eq!(value.as_i64(), Some(7));
    assert_eq!(value.as_bool(), None, "the kind follows the commit");

    // Text with no boolean or number reading: a string, kept verbatim.
    edit_parameter(&editor, 6, &adder, "b", Some("hi"));
    let value = &held_definition(&editor)["nodes"][0]["parameters"]["b"];
    assert_eq!(value.as_str(), Some("hi"));

    // The push reaches every connection with the whole updated definition.
    let mut watcher = editor.subscribe();
    edit_parameter(&editor, 7, &adder, "b", Some("2"));
    let pushed = push(&mut watcher);
    assert_eq!(pushed["type"], "definition");
    assert_eq!(
        pushed["graph"]["nodes"][0]["parameters"],
        json!({ "a": 7, "b": 2 })
    );
}

/// The same text hand-written into a file's parameters — the reference
/// reading a commit must match, read by the loader itself.
fn hand_written_file(text: &str) -> String {
    format!(
        "schema_version: 1\nnodes:\n  - uuid: b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33\n    type_ref: t\n    parameters: {{ a: {text} }}\nedges: []"
    )
}

#[test]
fn a_committed_text_stores_exactly_what_the_same_text_hand_written_stores() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");

    // Case-variant booleans, signed and radix integers, leading-zero
    // strings, the float forms, and prose — each commit read by the
    // loader's own reading of the same text in a file.
    for (id, text) in [
        "true", "True", "TRUE", "false", "FALSE", "7", "+7", "-3", "0x10", "0o17", "0b101", "07",
        "-007", "2.5", "+2.5", "1e3", ".5", "5.", "1e999", "hi",
    ]
    .iter()
    .enumerate()
    {
        edit_parameter(&editor, id as u64 + 1, &adder, "a", Some(text));
        let committed = &held_definition(&editor)["nodes"][0]["parameters"]["a"];
        let hand_written =
            graph::load(&hand_written_file(text)).expect("the hand-written text loads");
        assert_eq!(
            *committed,
            serde_json::to_value(&hand_written.nodes[0].parameters["a"]).unwrap(),
            "for the text {text:?}"
        );
    }

    // The refusals agree too: what the file format rejects, the commit
    // path rejects.
    for (id, text) in ["null", "~", "[1, 2]", "{a: 1}", "18446744073709551615"]
        .iter()
        .enumerate()
    {
        let refused = edit_parameter(&editor, id as u64 + 100, &adder, "a", Some(text));
        assert_eq!(refused["type"], "error", "for the text {text:?}");
        assert!(
            graph::load(&hand_written_file(text)).is_err(),
            "the file format refuses {text:?} too"
        );
    }
}

#[test]
fn committing_an_empty_field_removes_the_parameter() {
    let editor = editor_holding(SEEDED);
    let adder = "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33";

    let cleared = edit_parameter(&editor, 1, adder, "a", None);
    assert_eq!(cleared["type"], "parameter_set");
    let node = &held_definition(&editor)["nodes"][0];
    assert!(
        node.get("parameters").is_none(),
        "an unset input is unset, no empty-string stand-in: {node}"
    );

    // Clearing an already-unset input changes nothing and answers the same.
    let again = edit_parameter(&editor, 2, adder, "b", None);
    assert_eq!(again["type"], "parameter_set");
    assert!(held_definition(&editor)["nodes"][0]
        .get("parameters")
        .is_none());
}

#[test]
fn a_parameter_edit_for_a_connected_input_is_refused_and_names_the_input() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");
    let identity = create(&editor, 2, "beta/identity");
    send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "wire", "from": "{identity}", "from_port": "value", "to": "{adder}", "to_port": "a"}}"#
        ),
    );

    let refused = edit_parameter(&editor, 4, &adder, "a", Some("1"));
    assert_eq!(refused["type"], "error");
    let message = refused["error"].as_str().unwrap();
    assert!(
        message.contains("`a`"),
        "the error names the input: {message}"
    );
    assert!(message.contains(&adder), "and the node: {message}");

    let definition = held_definition(&editor);
    assert_eq!(
        definition["edges"].as_array().unwrap().len(),
        1,
        "the connection is untouched"
    );
    assert!(definition["nodes"][0].get("parameters").is_none());

    // The connection stays usable: another input of the same node takes
    // a value.
    let other = edit_parameter(&editor, 5, &adder, "b", Some("1"));
    assert_eq!(other["type"], "parameter_set");
    assert_eq!(
        held_definition(&editor)["nodes"][0]["parameters"],
        json!({ "b": 1 })
    );
}

#[test]
fn a_value_that_is_not_a_plain_scalar_is_refused_and_the_definition_is_untouched() {
    let editor = editor_holding(SEEDED);
    let adder = "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33";
    let before = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);

    for (id, text) in [
        (2, "null"),
        (3, "[1, 2]"),
        (4, "{nested: true}"),
        // An integer the file format cannot carry either.
        (5, "18446744073709551615"),
    ] {
        let reply = edit_parameter(&editor, id, adder, "b", Some(text));
        assert_eq!(reply["type"], "error", "for {text:?}");
        assert_eq!(reply["id"], id, "for {text:?}");
    }

    let after = send(&editor, r#"{"id": 6, "type": "get_definition"}"#);
    assert_eq!(
        before["graph"], after["graph"],
        "the definition is untouched"
    );
    assert_eq!(after["id"], 6, "the connection is still usable");
}

#[test]
fn an_input_name_the_type_does_not_declare_is_stored_not_refused() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");

    let edited = edit_parameter(&editor, 2, &adder, "x", Some("hi"));
    assert_eq!(edited["type"], "parameter_set");
    assert_eq!(
        held_definition(&editor)["nodes"][0]["parameters"],
        json!({ "x": "hi" }),
        "the file format carries it; compile time judges"
    );
}

#[test]
fn the_listing_carries_the_base_scalar_classification() {
    let editor = editor();
    let reply = send(&editor, r#"{"id": 1, "type": "list_node_types"}"#);
    let base_scalars = &reply["base_scalars"];
    assert_eq!(base_scalars["i32"], json!(true), "a base scalar");
    assert_eq!(base_scalars["String"], json!(true));
    assert_eq!(
        base_scalars["alpha/ratio"],
        json!(false),
        "a plugin's custom type is not a base scalar"
    );
    // The fact covers every registered type reference, base scalar or
    // not.
    for data_type in registry::data_types() {
        assert!(
            base_scalars[data_type.name].is_boolean(),
            "the fact covers {}: {base_scalars}",
            data_type.name
        );
    }
}

#[test]
fn malformed_label_and_parameter_edits_are_errors_and_leave_the_connection_usable() {
    let editor = editor();
    let adder = create(&editor, 1, "alpha/add");

    for (message, id) in [
        (
            r#"{"id": 2, "type": "set_label", "uuid": "not a uuid", "label": "x"}"#,
            2,
        ),
        (
            r#"{"id": 3, "type": "set_label", "uuid": "{adder}", "label": 3}"#,
            3,
        ),
        (r#"{"id": 4, "type": "set_label", "uuid": "{adder}"}"#, 4),
        (
            r#"{"id": 5, "type": "set_parameter", "uuid": "{adder}"}"#,
            5,
        ),
        (
            r#"{"id": 6, "type": "set_parameter", "uuid": "{adder}", "input": 3}"#,
            6,
        ),
        // The commit is the typed text: a non-string value is a misuse
        // of the message, not a value to read.
        (
            r#"{"id": 7, "type": "set_parameter", "uuid": "{adder}", "input": "a", "value": 3}"#,
            7,
        ),
        (
            r#"{"id": 8, "type": "set_parameter", "uuid": "{adder}", "input": "a", "value": "1", "extra": 1}"#,
            8,
        ),
    ] {
        let message = message.replace("{adder}", &adder);
        let reply = send(&editor, &message);
        assert_eq!(reply["type"], "error", "for {message}");
        assert_eq!(reply["id"], id, "for {message}");
    }

    let usable = send(&editor, r#"{"id": 9, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 9);
    assert!(
        usable["graph"]["nodes"][0].get("parameters").is_none(),
        "nothing landed"
    );
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

const SECOND: &str = "schema_version: 1
name: second
nodes:
  - uuid: 0a6b3e72-9c15-4d8f-b3e7-4c8a1f6d9b23
    type_ref: beta/identity
    metadata:
      position: { x: 30, y: 40 }
edges: []";

/// The file state the editor holds: the current file and the
/// unsaved-changes marker, as any connection sees it.
fn held_file(editor: &Editor) -> Value {
    send(editor, r#"{"id": 0, "type": "get_definition"}"#)["file"].clone()
}

#[test]
fn launching_on_a_file_seeds_the_definition_and_names_it() {
    let file = written_graph("launch", SEEDED);
    let path = file.to_string_lossy().into_owned();
    let editor = Editor::new(graph::load(SEEDED).unwrap(), Some(path.clone()));

    let reply = send(&editor, r#"{"id": 1, "type": "get_definition"}"#);
    assert_eq!(reply["graph"]["name"], "seeded");
    assert_eq!(
        reply["file"],
        json!({ "path": path, "dirty": false }),
        "the launched file is the current one, the definition clean"
    );

    let _ = fs::remove_file(&file);
}

#[test]
fn open_replaces_the_definition_and_pushes_it_to_every_connection() {
    let editor = editor_holding(SEEDED);
    let file = written_graph("open", SECOND);
    let path = file.to_string_lossy().into_owned();
    let mut watchers = [editor.subscribe(), editor.subscribe()];

    let opened = send(
        &editor,
        &format!(
            r#"{{"id": 1, "type": "open_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(opened["type"], "file_opened");
    assert_eq!(opened["id"], 1);

    // The opened definition is the loader's, whole: the same one the
    // launch door seeds from the same text — nodes, parameters,
    // metadata, edges — so the round trip rides on open placing it
    // verbatim.
    let definition = held_definition(&editor);
    assert_eq!(definition, held_definition(&editor_holding(SECOND)));
    assert_eq!(
        held_file(&editor),
        json!({ "path": path, "dirty": false }),
        "the opened file is the current one, the definition clean"
    );
    for watcher in &mut watchers {
        let pushed = push(watcher);
        assert_eq!(pushed["type"], "definition");
        assert_eq!(
            pushed["graph"], definition,
            "the push carries the whole opened definition"
        );
        assert_eq!(pushed["file"], json!({ "path": path, "dirty": false }));
    }

    let _ = fs::remove_file(&file);
}

#[test]
fn a_failed_open_names_the_path_and_leaves_the_session_untouched() {
    let file = written_graph("current", SEEDED);
    let path = file.to_string_lossy().into_owned();
    let editor = Editor::new(graph::load(SEEDED).unwrap(), Some(path.clone()));
    create(&editor, 1, "alpha/add");
    let before = held_definition(&editor);
    let broken = written_graph(
        "broken",
        "schema_version: 1\nnodes: []\nedges: []\nsurprise: true\n",
    );

    // A path that cannot be read reports the read failure; content the
    // loader rejects reports the load error, naming what and where.
    let missing = send(
        &editor,
        r#"{"id": 2, "type": "open_file", "path": "/nodetool-missing-dir/graph.yml"}"#,
    );
    assert_eq!(missing["type"], "error");
    assert!(
        missing["error"]
            .as_str()
            .unwrap()
            .contains("/nodetool-missing-dir/graph.yml"),
        "names the path: {missing}"
    );

    let refused = send(
        &editor,
        &format!(
            r#"{{"id": 3, "type": "open_file", "path": {}}}"#,
            path_field(&broken)
        ),
    );
    assert_eq!(refused["type"], "error");
    let message = refused["error"].as_str().unwrap();
    assert!(
        message.contains(&broken.to_string_lossy().into_owned()),
        "names the file: {message}"
    );
    assert!(message.contains("surprise"), "names the fault: {message}");

    assert_eq!(
        held_definition(&editor),
        before,
        "the held definition is untouched"
    );
    assert_eq!(
        held_file(&editor),
        json!({ "path": path, "dirty": true }),
        "the current file and the unsaved state are untouched"
    );
    let usable = send(&editor, r#"{"id": 4, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 4, "the connection is still usable");

    let _ = fs::remove_file(&file);
    let _ = fs::remove_file(&broken);
}

/// A file the loader accepts but the browser's JSON push cannot carry:
/// its metadata holds a non-finite float key.
const UNCARRIABLE: &str = "schema_version: 1
nodes:
  - uuid: 0a6b3e72-9c15-4d8f-b3e7-4c8a1f6d9b23
    type_ref: t
    metadata:
      .inf: note
edges: []";

#[test]
fn an_open_of_a_file_the_browser_cannot_carry_fails_and_leaves_the_session_untouched() {
    let file = written_graph("current", SEEDED);
    let path = file.to_string_lossy().into_owned();
    let editor = Editor::new(graph::load(SEEDED).unwrap(), Some(path.clone()));
    let before = held_definition(&editor);
    let uncarriable = written_graph("uncarriable", UNCARRIABLE);

    let refused = send(
        &editor,
        &format!(
            r#"{{"id": 1, "type": "open_file", "path": {}}}"#,
            path_field(&uncarriable)
        ),
    );
    assert_eq!(refused["type"], "error");
    let message = refused["error"].as_str().unwrap();
    assert!(
        message.contains(&uncarriable.to_string_lossy().into_owned()),
        "names the file: {message}"
    );
    assert!(message.contains("key"), "names the fault: {message}");

    assert_eq!(
        held_definition(&editor),
        before,
        "the held definition is untouched"
    );
    assert_eq!(
        held_file(&editor),
        json!({ "path": path, "dirty": false }),
        "the current file and the clean state are untouched"
    );
    let usable = send(&editor, r#"{"id": 2, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 2, "the connection is still usable");

    let _ = fs::remove_file(&file);
    let _ = fs::remove_file(&uncarriable);
}

#[test]
fn save_writes_the_document_the_definition_dumps() {
    let editor = editor_holding(SEEDED);
    let file = temp_path("save");

    let saved = send(
        &editor,
        &format!(
            r#"{{"id": 1, "type": "save_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(saved["type"], "file_saved");
    assert_eq!(saved["id"], 1);

    let held = graph::load(SEEDED).unwrap();
    let written = fs::read_to_string(&file).expect("the save wrote the file");
    assert_eq!(
        written,
        graph::dump(&held),
        "the file is the definition through the format's own dump, nothing invented"
    );
    assert_eq!(
        graph::load(&written).unwrap(),
        held,
        "the format's own round-trip fidelity holds through the editor's door"
    );

    let _ = fs::remove_file(&file);
}

#[test]
fn a_save_retargets_the_current_file_and_pushes_the_state_to_every_connection() {
    let editor = editor();
    let first = written_graph("first", SECOND);
    let second = temp_path("second");
    let mut watchers = [editor.subscribe(), editor.subscribe()];

    // The first save of an untitled graph names the file; the answer
    // becomes the graph's file once the save succeeds.
    let saved = send(
        &editor,
        &format!(
            r#"{{"id": 1, "type": "save_file", "path": {}}}"#,
            path_field(&first)
        ),
    );
    assert_eq!(saved["type"], "file_saved");
    for watcher in &mut watchers {
        let pushed = push(watcher);
        assert_eq!(pushed["type"], "file");
        assert_eq!(pushed["path"], first.to_string_lossy().into_owned());
        assert_eq!(pushed["dirty"], json!(false));
    }

    // An edit dirties; the plain save writes the current file again,
    // naming no path.
    let adder = create(&editor, 2, "alpha/add");
    let saved = send(&editor, r#"{"id": 3, "type": "save_file"}"#);
    assert_eq!(saved["type"], "file_saved");
    let written = fs::read_to_string(&first).unwrap();
    assert!(
        written.contains(&adder),
        "the edit reached the file: {written}"
    );
    assert_eq!(
        held_file(&editor),
        json!({ "path": first.to_string_lossy().into_owned(), "dirty": false })
    );

    // Saving elsewhere re-targets the same way, one save mechanism.
    let saved = send(
        &editor,
        &format!(
            r#"{{"id": 4, "type": "save_file", "path": {}}}"#,
            path_field(&second)
        ),
    );
    assert_eq!(saved["type"], "file_saved");
    assert_eq!(
        held_file(&editor),
        json!({ "path": second.to_string_lossy().into_owned(), "dirty": false })
    );
    assert_eq!(
        fs::read_to_string(&second).unwrap(),
        fs::read_to_string(&first).unwrap(),
        "the same document landed at both files"
    );

    let _ = fs::remove_file(&first);
    let _ = fs::remove_file(&second);
}

#[test]
fn a_first_save_without_a_path_is_refused() {
    let editor = editor();
    let refused = send(&editor, r#"{"id": 1, "type": "save_file"}"#);
    assert_eq!(refused["type"], "error");
    assert!(
        refused["error"].as_str().unwrap().contains("path"),
        "the error says what is missing: {refused}"
    );
    assert_eq!(held_file(&editor), json!({ "path": null, "dirty": false }));
}

#[test]
fn a_failed_save_names_the_path_and_leaves_the_session_untouched() {
    let file = written_graph("save-fail", SEEDED);
    let path = file.to_string_lossy().into_owned();
    let editor = Editor::new(graph::load(SEEDED).unwrap(), Some(path.clone()));
    create(&editor, 1, "alpha/add");
    let before = held_definition(&editor);

    let refused = send(
        &editor,
        r#"{"id": 2, "type": "save_file", "path": "/nodetool-missing-dir/graph.yml"}"#,
    );
    assert_eq!(refused["type"], "error");
    let message = refused["error"].as_str().unwrap();
    assert!(
        message.contains("/nodetool-missing-dir/graph.yml"),
        "names the path: {message}"
    );

    assert_eq!(
        held_definition(&editor),
        before,
        "the held definition is untouched"
    );
    assert_eq!(
        held_file(&editor),
        json!({ "path": path, "dirty": true }),
        "the current file and the unsaved state are untouched"
    );
    let usable = send(&editor, r#"{"id": 3, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 3, "the connection is still usable");

    let _ = fs::remove_file(&file);
}

#[test]
fn the_dirty_rule_edits_set_it_and_open_save_and_fresh_clear_it() {
    let editor = editor();
    let mut watcher = editor.subscribe();

    // An edit sets it, and the state travels beside the definition it
    // belongs to.
    let adder = create(&editor, 1, "alpha/add");
    assert_eq!(held_file(&editor)["dirty"], json!(true));
    let pushed = push(&mut watcher);
    assert_eq!(pushed["type"], "definition");
    assert_eq!(pushed["file"]["dirty"], json!(true));

    // A save clears it.
    let file = temp_path("dirty");
    send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "save_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(held_file(&editor)["dirty"], json!(false));

    // An edit that changes nothing sets it not: unhooking an input with
    // no wire leaves the marker as it was.
    let unhooked = send(
        &editor,
        &format!(r#"{{"id": 3, "type": "unhook", "to": "{adder}", "to_port": "b"}}"#),
    );
    assert_eq!(unhooked["type"], "unhooked");
    assert_eq!(
        held_file(&editor)["dirty"],
        json!(false),
        "a no-op edit sets nothing"
    );

    // An open — the current file, even — replaces the graph and clears it.
    create(&editor, 3, "alpha/add");
    send(
        &editor,
        &format!(
            r#"{{"id": 4, "type": "open_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(
        held_file(&editor),
        json!({ "path": file.to_string_lossy().into_owned(), "dirty": false })
    );

    // The fresh-graph control clears it and returns to untitled in one
    // step.
    create(&editor, 5, "alpha/add");
    let created = send(&editor, r#"{"id": 6, "type": "new_graph"}"#);
    assert_eq!(created["type"], "graph_created");
    assert_eq!(created["id"], 6);
    assert_eq!(held_file(&editor), json!({ "path": null, "dirty": false }));
    let definition = held_definition(&editor);
    assert_eq!(definition["nodes"], json!([]));
    assert_eq!(definition["edges"], json!([]));

    let _ = fs::remove_file(&file);
}

#[test]
fn malformed_file_operations_are_errors_and_leave_the_connection_usable() {
    let editor = editor();

    for (message, id) in [
        (r#"{"id": 1, "type": "open_file"}"#, 1),
        (r#"{"id": 2, "type": "open_file", "path": 3}"#, 2),
        (
            r#"{"id": 3, "type": "open_file", "path": "x", "extra": true}"#,
            3,
        ),
        (r#"{"id": 4, "type": "save_file", "path": 3}"#, 4),
        (
            r#"{"id": 5, "type": "save_file", "path": "x", "extra": true}"#,
            5,
        ),
        (r#"{"id": 6, "type": "new_graph", "extra": true}"#, 6),
    ] {
        let reply = send(&editor, message);
        assert_eq!(reply["type"], "error", "for {message}");
        assert_eq!(reply["id"], id, "for {message}");
    }

    let usable = send(&editor, r#"{"id": 7, "type": "get_definition"}"#);
    assert_eq!(usable["id"], 7, "the connection is still usable");
    assert_eq!(usable["graph"]["nodes"], json!([]), "nothing landed");
}
