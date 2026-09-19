//! The packaging gestures the server derives: the package operation
//! turning a selection into a group definition and one collapsed instance,
//! the unpack operation dissolving an instance back into its nodes, and
//! the refusals that leave the definition exactly as it was. The file
//! operations and the per-node edits are server.rs's; the run lock's
//! refusal of these two rides the lock test there.

mod server_common;

use nodetool::graph;
use nodetool::server::Editor;
use serde_json::{json, Value};
use server_common::*;
use test_plugin_alpha as _;
use test_plugin_beta as _;

fn push(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> Value {
    serde_json::from_str(&receiver.blocking_recv().expect("a push arrives"))
        .expect("a push is JSON")
}

const U_SOURCE: &str = "00000000-0000-0000-0000-0000000000c1";
const U_ADD_1: &str = "00000000-0000-0000-0000-0000000000a1";
const U_ADD_2: &str = "00000000-0000-0000-0000-0000000000a2";
const U_SINK: &str = "00000000-0000-0000-0000-0000000000b1";

/// A stage worth packaging: two adds wired together inside the selection,
/// the first fed from an outside source and the second feeding an outside
/// sink — one crossing edge each way.
const STAGED: &str = "schema_version: 2
name: staged
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000c1
    type_ref: beta/identity
    metadata:
      position: { x: -100, y: 0 }
  - uuid: 00000000-0000-0000-0000-0000000000a1
    type_ref: alpha/add
    parameters:
      a: 2
    metadata:
      position: { x: 0, y: 0 }
  - uuid: 00000000-0000-0000-0000-0000000000a2
    type_ref: alpha/add
    metadata:
      position: { x: 100, y: 0 }
  - uuid: 00000000-0000-0000-0000-0000000000b1
    type_ref: beta/identity
    metadata:
      position: { x: 300, y: 0 }
edges:
  - from: 00000000-0000-0000-0000-0000000000c1
    from_port: value
    to: 00000000-0000-0000-0000-0000000000a1
    to_port: b
  - from: 00000000-0000-0000-0000-0000000000a1
    from_port: sum
    to: 00000000-0000-0000-0000-0000000000a2
    to_port: b
  - from: 00000000-0000-0000-0000-0000000000a2
    from_port: sum
    to: 00000000-0000-0000-0000-0000000000b1
    to_port: value";

fn package(editor: &Editor, id: u64, nodes: &[&str], name: &str) -> Value {
    let list = nodes
        .iter()
        .map(|uuid| format!("\"{uuid}\""))
        .collect::<Vec<_>>()
        .join(", ");
    send(
        editor,
        &format!(r#"{{"id": {id}, "type": "package_group", "nodes": [{list}], "name": "{name}"}}"#),
    )
}

fn unpack(editor: &Editor, id: u64, uuid: &str) -> Value {
    send(
        editor,
        &format!(r#"{{"id": {id}, "type": "unpack_group", "uuid": "{uuid}"}}"#),
    )
}

fn group_of(definition: &Value, name: &str) -> Value {
    definition["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["name"] == name)
        .expect("the definition carries the group")
        .clone()
}

fn node_of<'v>(definition: &'v Value, uuid: &str) -> &'v Value {
    definition["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["uuid"] == uuid)
        .expect("the definition carries the node")
}

#[test]
fn packaging_derives_the_group_from_the_held_definition() {
    let editor = editor_holding(STAGED);
    let mut watcher = editor.subscribe();

    let reply = package(&editor, 1, &[U_ADD_1, U_ADD_2], "stage");
    assert_eq!(reply["type"], "group_packaged");
    let instance = reply["uuid"].as_str().unwrap();

    let definition = held_definition(&editor);
    // The selection collapsed to one instance sitting at the selection's
    // centroid, typed by the group's name, the packaged nodes gone from
    // the top level.
    let nodes = definition["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3, "the source and the sink remain beside it");
    let collapsed = node_of(&definition, instance);
    assert_eq!(collapsed["type_ref"], "stage");
    assert_eq!(
        collapsed["metadata"]["position"],
        json!({ "x": 50, "y": 0 })
    );

    // The group: one exposed input and one exposed output binding the
    // inner ports the crossing edges stood in for; the internal edge and
    // the parameters travel into the body; the stored positions move in
    // as offsets from the centroid.
    let group = group_of(&definition, "stage");
    assert_eq!(
        group["inputs"],
        json!([{ "name": "b", "type_refs": ["i32"], "node": U_ADD_1, "port": "b" }])
    );
    assert_eq!(
        group["outputs"],
        json!([{ "name": "sum", "type_refs": ["i32"], "node": U_ADD_2, "port": "sum" }])
    );
    assert_eq!(
        node_of(&group, U_ADD_1)["metadata"]["position"],
        json!({ "x": -50, "y": 0 })
    );
    assert_eq!(node_of(&group, U_ADD_1)["parameters"], json!({ "a": 2 }));
    assert_eq!(
        node_of(&group, U_ADD_2)["metadata"]["position"],
        json!({ "x": 50, "y": 0 })
    );
    assert_eq!(
        group["edges"],
        json!([{ "from": U_ADD_1, "from_port": "sum", "to": U_ADD_2, "to_port": "b" }])
    );

    // The top level means what it meant: the outside wires now land on the
    // collapsed node's exposed ports.
    assert_eq!(
        definition["edges"],
        json!([
            { "from": U_SOURCE, "from_port": "value", "to": instance, "to_port": "b" },
            { "from": instance, "from_port": "sum", "to": U_SINK, "to_port": "value" },
        ])
    );

    // One atomic edit, pushed whole to every connection.
    let pushed = push(&mut watcher);
    assert_eq!(pushed["type"], "definition");
    assert_eq!(pushed["graph"]["groups"][0]["name"], "stage");
    assert_eq!(pushed["graph"]["nodes"].as_array().unwrap().len(), 3);
}

#[test]
fn a_selection_of_one_with_no_crossing_edges_packages_into_a_group_with_no_ports() {
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {U_ADD_1}
    type_ref: alpha/add
    metadata:
      position: {{ x: 40, y: -20 }}
edges: []"
    );
    let editor = editor_holding(&text);
    let reply = package(&editor, 1, &[U_ADD_1], "keep");
    assert_eq!(reply["type"], "group_packaged");
    let instance = reply["uuid"].as_str().unwrap();

    let definition = held_definition(&editor);
    let group = group_of(&definition, "keep");
    assert_eq!(group["inputs"], json!([]));
    assert_eq!(group["outputs"], json!([]));
    assert_eq!(group["edges"], json!([]));
    assert_eq!(
        node_of(&group, U_ADD_1)["metadata"]["position"],
        json!({ "x": 0, "y": 0 }),
        "a one-node selection's centroid is the node's own position, the offset zero"
    );
    assert_eq!(
        node_of(&definition, instance)["metadata"]["position"],
        json!({ "x": 40, "y": -20 })
    );
    assert!(definition["edges"].as_array().unwrap().is_empty());
}

#[test]
fn derived_port_names_dedupe_across_the_new_port_set() {
    // Two identities packaged between outside wires: both their inputs and
    // their outputs carry the same inner port name, so the derived ports
    // share one namespace and the later comers take the numbered names.
    const D0: &str = "00000000-0000-0000-0000-0000000000d0";
    const D1: &str = "00000000-0000-0000-0000-0000000000d1";
    const D2: &str = "00000000-0000-0000-0000-0000000000d2";
    const D4: &str = "00000000-0000-0000-0000-0000000000d4";
    const D5: &str = "00000000-0000-0000-0000-0000000000d5";
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {D0}
    type_ref: beta/identity
    metadata:
      position: {{ x: -200, y: 0 }}
  - uuid: {D1}
    type_ref: beta/identity
    metadata:
      position: {{ x: 0, y: 0 }}
  - uuid: {D2}
    type_ref: beta/identity
    metadata:
      position: {{ x: 100, y: 0 }}
  - uuid: {D4}
    type_ref: beta/identity
    metadata:
      position: {{ x: 300, y: -100 }}
  - uuid: {D5}
    type_ref: beta/identity
    metadata:
      position: {{ x: 300, y: 100 }}
edges:
  - from: {D0}
    from_port: value
    to: {D1}
    to_port: value
  - from: {D0}
    from_port: value
    to: {D2}
    to_port: value
  - from: {D1}
    from_port: value
    to: {D4}
    to_port: value
  - from: {D2}
    from_port: value
    to: {D5}
    to_port: value"
    );
    let editor = editor_holding(&text);
    let reply = package(&editor, 1, &[D1, D2], "chained");
    assert_eq!(reply["type"], "group_packaged");

    let group = group_of(&held_definition(&editor), "chained");
    assert_eq!(
        group["inputs"],
        json!([
            { "name": "value", "type_refs": ["i32", "f64"], "node": D1, "port": "value" },
            { "name": "value 2", "type_refs": ["i32", "f64"], "node": D2, "port": "value" },
        ])
    );
    assert_eq!(
        group["outputs"],
        json!([
            { "name": "value 3", "type_refs": ["i32", "f64"], "node": D1, "port": "value" },
            { "name": "value 4", "type_refs": ["i32", "f64"], "node": D2, "port": "value" },
        ]),
        "one namespace across the whole new port set, first come keeping the plain name"
    );
}

#[test]
fn a_node_without_a_stored_position_contributes_neither_centroid_nor_offset() {
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {U_ADD_1}
    type_ref: alpha/add
    metadata:
      position: {{ x: 0, y: 40 }}
  - uuid: {U_ADD_2}
    type_ref: alpha/add
edges: []"
    );
    let editor = editor_holding(&text);
    let reply = package(&editor, 1, &[U_ADD_1, U_ADD_2], "partial");
    assert_eq!(reply["type"], "group_packaged");
    let instance = reply["uuid"].as_str().unwrap();

    let definition = held_definition(&editor);
    assert_eq!(
        node_of(&definition, instance)["metadata"]["position"],
        json!({ "x": 0, "y": 40 }),
        "the centroid is the positioned node's alone"
    );
    let group = group_of(&definition, "partial");
    assert_eq!(
        node_of(&group, U_ADD_1)["metadata"]["position"],
        json!({ "x": 0, "y": 0 })
    );
    assert!(
        node_of(&group, U_ADD_2).get("metadata").is_none(),
        "the placeless node carries no position into the body either"
    );
}

#[test]
fn a_name_duplicating_a_document_group_is_refused_and_the_definition_is_untouched() {
    let editor = editor_holding(STAGED);
    let first = package(&editor, 1, &[U_ADD_1, U_ADD_2], "stage");
    assert_eq!(first["type"], "group_packaged");
    let before = held_definition(&editor);

    let again = package(&editor, 2, &[U_SINK], "stage");
    assert_eq!(again["type"], "error");
    let message = again["error"].as_str().unwrap();
    assert!(
        message.contains("`stage`"),
        "the refusal names the clash: {message}"
    );
    assert_eq!(
        held_definition(&editor),
        before,
        "the definition is exactly as it was and the connection stayed usable"
    );
}

#[test]
fn a_name_colliding_with_a_linked_type_reference_is_refused() {
    let editor = editor_holding(STAGED);
    let before = held_definition(&editor);
    let reply = package(&editor, 1, &[U_ADD_1, U_ADD_2], "alpha/add");
    assert_eq!(reply["type"], "error");
    let message = reply["error"].as_str().unwrap();
    assert!(
        message.contains("alpha/add"),
        "the refusal names the linked type it collides with: {message}"
    );
    assert_eq!(held_definition(&editor), before);
}

#[test]
fn a_crossing_edge_at_a_placeholder_inside_the_selection_blocks_naming_the_node() {
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {U_SOURCE}
    type_ref: beta/identity
    metadata:
      position: {{ x: -100, y: 0 }}
  - uuid: {U_ADD_1}
    type_ref: gone/missing
    metadata:
      position: {{ x: 0, y: 0 }}
  - uuid: {U_ADD_2}
    type_ref: alpha/add
    metadata:
      position: {{ x: 100, y: 0 }}
edges:
  - from: {U_SOURCE}
    from_port: value
    to: {U_ADD_1}
    to_port: in"
    );
    let editor = editor_holding(&text);
    let before = held_definition(&editor);
    let reply = package(&editor, 1, &[U_ADD_1, U_ADD_2], "stage");
    assert_eq!(reply["type"], "error");
    let message = reply["error"].as_str().unwrap();
    assert!(
        message.contains(U_ADD_1) && message.contains("gone/missing"),
        "the refusal names the node whose port would carry nothing: {message}"
    );
    assert_eq!(held_definition(&editor), before);

    // The same placeholder whose edges stay internal packages fine: nobody
    // asks its ports anything.
    let internal = format!(
        "schema_version: 2
nodes:
  - uuid: {U_ADD_1}
    type_ref: gone/missing
    metadata:
      position: {{ x: 0, y: 0 }}
  - uuid: {U_ADD_2}
    type_ref: alpha/add
    metadata:
      position: {{ x: 100, y: 0 }}
edges:
  - from: {U_ADD_2}
    from_port: sum
    to: {U_ADD_1}
    to_port: in"
    );
    let editor = editor_holding(&internal);
    let reply = package(&editor, 1, &[U_ADD_1, U_ADD_2], "quiet");
    assert_eq!(reply["type"], "group_packaged", "no crossing, no question");
}

#[test]
fn a_placeholder_outside_the_selection_does_not_block() {
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {U_ADD_1}
    type_ref: alpha/add
    metadata:
      position: {{ x: 0, y: 0 }}
  - uuid: {U_ADD_2}
    type_ref: gone/missing
    metadata:
      position: {{ x: 200, y: 0 }}
edges:
  - from: {U_ADD_1}
    from_port: sum
    to: {U_ADD_2}
    to_port: in"
    );
    let editor = editor_holding(&text);
    let reply = package(&editor, 1, &[U_ADD_1], "stage");
    assert_eq!(
        reply["type"], "group_packaged",
        "the exposed port declares the packaged side's types; the placeholder stays compile's"
    );
    let instance = reply["uuid"].as_str().unwrap();
    let definition = held_definition(&editor);
    let group = group_of(&definition, "stage");
    assert_eq!(
        group["outputs"][0]["type_refs"],
        json!(["i32"]),
        "the exposed port declares the inner output's type references"
    );
    assert_eq!(
        definition["edges"],
        json!([{ "from": instance, "from_port": "sum", "to": U_ADD_2, "to_port": "in" }])
    );
}

#[test]
fn unpacking_splices_the_wires_back_and_lands_the_nodes_around_where_the_group_sat() {
    let editor = editor_holding(STAGED);
    let packaged = package(&editor, 1, &[U_ADD_1, U_ADD_2], "stage");
    let instance = packaged["uuid"].as_str().unwrap().to_owned();

    // The user moves the group on: the round trip is from where it ended,
    // not where the nodes were first drawn.
    send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "move_node", "uuid": "{instance}", "position": {{"x": 1000, "y": 500}}}}"#
        ),
    );

    let mut watcher = editor.subscribe();
    let reply = unpack(&editor, 3, &instance);
    assert_eq!(reply["type"], "group_unpacked");
    let returned = reply["nodes"].as_array().unwrap();
    assert_eq!(returned, json!([U_ADD_1, U_ADD_2]).as_array().unwrap());

    let definition = held_definition(&editor);
    assert_eq!(
        node_of(&definition, U_ADD_1)["metadata"]["position"],
        json!({ "x": 950, "y": 500 }),
        "each node lands at its stored offset from where the instance sat"
    );
    assert_eq!(
        node_of(&definition, U_ADD_2)["metadata"]["position"],
        json!({ "x": 1050, "y": 500 })
    );
    assert_eq!(
        node_of(&definition, U_ADD_1)["parameters"],
        json!({ "a": 2 }),
        "the body's own carriage returns with it"
    );
    assert_eq!(
        definition["edges"],
        json!([
            { "from": U_SOURCE, "from_port": "value", "to": U_ADD_1, "to_port": "b" },
            { "from": U_ADD_2, "from_port": "sum", "to": U_SINK, "to_port": "value" },
            { "from": U_ADD_1, "from_port": "sum", "to": U_ADD_2, "to_port": "b" },
        ]),
        "the wires re-attach through the bindings, the graph meaning what it meant"
    );
    assert!(
        definition["groups"].is_null(),
        "the definition leaves with its last instance: no ghost the canvas never shows"
    );

    let pushed = push(&mut watcher);
    assert_eq!(pushed["type"], "definition");
    assert_eq!(pushed["graph"]["nodes"].as_array().unwrap().len(), 4);
}

#[test]
fn an_exposed_input_value_lands_on_the_bound_inner_input_as_its_parameter() {
    let editor = editor_holding(STAGED);
    let packaged = package(&editor, 1, &[U_ADD_1, U_ADD_2], "stage");
    let instance = packaged["uuid"].as_str().unwrap().to_owned();

    // The user unhooks the group's exposed input and types a value there —
    // a literal given to the group, the way 23's inline field commits it.
    let unhooked = send(
        &editor,
        &format!(r#"{{"id": 2, "type": "unhook", "to": "{instance}", "to_port": "b"}}"#),
    );
    assert_eq!(unhooked["type"], "unhooked");
    let set = edit_parameter(&editor, 3, &instance, "b", Some("7"));
    assert_eq!(set["type"], "parameter_set");

    let reply = unpack(&editor, 4, &instance);
    assert_eq!(reply["type"], "group_unpacked");
    let definition = held_definition(&editor);
    assert_eq!(
        node_of(&definition, U_ADD_1)["parameters"],
        json!({ "a": 2, "b": 7 }),
        "the value the user gave the group's input is the value the inner node shows"
    );
    assert!(
        !definition["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|edge| edge["to"] == U_ADD_1 && edge["to_port"] == "b"),
        "the value lands as a parameter; the input carries one or the other"
    );
}

#[test]
fn a_returning_uuid_that_collides_with_a_surviving_node_is_reassigned() {
    // A hand-written file may carry a body uuid the top level also holds —
    // the loader checks each scope's uniqueness alone. Unpacking still
    // dissolves the instance: the returning node takes a fresh uuid and
    // its wires follow it.
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {U_ADD_1}
    type_ref: stage
    metadata:
      position: {{ x: 500, y: 0 }}
  - uuid: {U_SINK}
    type_ref: alpha/add
    metadata:
      position: {{ x: 1000, y: 0 }}
edges:
  - from: {U_SINK}
    from_port: sum
    to: {U_ADD_1}
    to_port: a
groups:
  - name: stage
    inputs:
      - name: a
        type_refs: [i32]
        node: {U_SINK}
        port: a
    outputs: []
    nodes:
      - uuid: {U_SINK}
        type_ref: alpha/add
        metadata:
          position: {{ x: 0, y: 0 }}
    edges: []"
    );
    let editor = editor_holding(&text);
    let reply = unpack(&editor, 1, U_ADD_1);
    assert_eq!(reply["type"], "group_unpacked");
    let returned = reply["nodes"].as_array().unwrap();
    assert_eq!(returned.len(), 1);
    assert_ne!(
        returned[0], U_SINK,
        "the survivor keeps its identity; the returning node takes a fresh one"
    );

    let definition = held_definition(&editor);
    assert_eq!(
        definition["edges"],
        json!([{ "from": U_SINK, "from_port": "sum", "to": returned[0], "to_port": "a" }]),
        "the wire follows the reassigned node"
    );
    assert!(
        definition["groups"].is_null(),
        "the returning node is the only reference left, so the definition still leaves"
    );
}

#[test]
fn the_definition_is_kept_while_other_instances_reference_it() {
    let one = "00000000-0000-0000-0000-0000000000f1";
    let two = "00000000-0000-0000-0000-0000000000f2";
    let text = format!(
        "schema_version: 2
nodes:
  - uuid: {one}
    type_ref: stage
    metadata:
      position: {{ x: 50, y: 0 }}
  - uuid: {two}
    type_ref: stage
    metadata:
      position: {{ x: 400, y: 0 }}
  - uuid: {U_SINK}
    type_ref: beta/identity
    metadata:
      position: {{ x: 800, y: 0 }}
edges:
  - from: {two}
    from_port: sum
    to: {U_SINK}
    to_port: value
groups:
  - name: stage
    inputs:
      - name: a
        type_refs: [i32]
        node: {U_ADD_1}
        port: a
    outputs:
      - name: sum
        type_refs: [i32]
        node: {U_ADD_2}
        port: sum
    nodes:
      - uuid: {U_ADD_1}
        type_ref: alpha/add
      - uuid: {U_ADD_2}
        type_ref: alpha/add
    edges:
      - from: {U_ADD_1}
        from_port: sum
        to: {U_ADD_2}
        to_port: b"
    );
    let editor = editor_holding(&text);

    let reply = unpack(&editor, 1, one);
    assert_eq!(reply["type"], "group_unpacked");
    let definition = held_definition(&editor);
    assert_eq!(
        group_of(&definition, "stage")["nodes"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "the other instance keeps the definition, theirs"
    );

    let again = unpack(&editor, 2, two);
    assert_eq!(again["type"], "group_unpacked");
    let definition = held_definition(&editor);
    assert!(
        definition["groups"].is_null(),
        "the last instance takes the definition with it"
    );
    // The second unpack's returning uuids were reassigned — the first
    // unpack's nodes hold their body uuids now — and its wires follow:
    // the external downstream re-seated on the exposed output's binding,
    // the inner edge between the returning pair.
    let edges = definition["edges"].as_array().unwrap();
    let returned = again["nodes"].as_array().unwrap();
    assert_eq!(edges.len(), 3);
    assert_eq!(
        edges.iter().find(|edge| edge["to"] == U_SINK).unwrap(),
        &json!({ "from": returned[1], "from_port": "sum", "to": U_SINK, "to_port": "value" }),
        "the second instance's wire re-attached through the binding too"
    );
    assert_eq!(
        edges
            .iter()
            .find(|edge| edge["from"] == returned[0])
            .unwrap(),
        &json!({ "from": returned[0], "from_port": "sum", "to": returned[1], "to_port": "b" })
    );
}

#[test]
fn a_packaged_graph_survives_the_file_round_trip_and_unpacks_the_same() {
    let editor = editor_holding(STAGED);
    let packaged = package(&editor, 1, &[U_ADD_1, U_ADD_2], "stage");
    let instance = packaged["uuid"].as_str().unwrap().to_owned();
    let file = temp_path("packaged-round-trip");
    let saved = send(
        &editor,
        &format!(
            r#"{{"id": 2, "type": "save_file", "path": {}}}"#,
            path_field(&file)
        ),
    );
    assert_eq!(saved["type"], "file_saved");

    let reopened = Editor::new(read_file(&file), None);
    let definition = held_definition(&reopened);
    let group = group_of(&definition, "stage");
    assert_eq!(group["outputs"][0]["port"], "sum");
    assert_eq!(group["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(
        definition["edges"].as_array().unwrap().len(),
        2,
        "the reopened file carries the group verbatim, wires landing on its ports"
    );

    let reply = unpack(&reopened, 1, &instance);
    assert_eq!(reply["type"], "group_unpacked");
    let definition = held_definition(&reopened);
    assert_eq!(definition["edges"].as_array().unwrap().len(), 3);
    let _ = std::fs::remove_file(&file);
}

/// The definition a saved file holds, read the way the editor's launch
/// does — the loader's parse, nothing else.
fn read_file(path: &std::path::Path) -> graph::GraphDefinition {
    graph::load(&std::fs::read_to_string(path).expect("the test saved the file"))
        .expect("the saved file loads")
}
