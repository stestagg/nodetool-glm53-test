//! The editor server's operations against the held definition: the
//! listing, the create, move, label, parameter, wire, unhook, and delete
//! operations, and every malformed operation path. The transport — the
//! greeting, the envelope, the served socket — is connection.rs's.

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
/// wire, label, parameter, unhook, and delete tests build graphs from,
/// positions being incidental to them.
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

/// Set or clear a node's label override; the empty label means the type's
/// default.
fn edit_label(editor: &Editor, id: u64, uuid: &str, label: &str) -> Value {
    send(
        editor,
        &format!(r#"{{"id": {id}, "type": "set_label", "uuid": "{uuid}", "label": "{label}"}}"#),
    )
}

/// Set or clear an input's parameter value. The value is the typed text,
/// carried as a JSON string and read server-side as the file format
/// reads it; `None` commits the empty field, meaning unset.
fn edit_parameter(editor: &Editor, id: u64, uuid: &str, input: &str, value: Option<&str>) -> Value {
    let value = value
        .map(|text| format!(r#", "value": {}"#, serde_json::to_string(text).unwrap()))
        .unwrap_or_default();
    send(
        editor,
        &format!(
            r#"{{"id": {id}, "type": "set_parameter", "uuid": "{uuid}", "input": "{input}"{value}}}"#
        ),
    )
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
