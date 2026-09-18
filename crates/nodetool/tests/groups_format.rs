//! The groups section of the file format: the round trip, the version
//! gate, and the document-local load errors — the checks the loader makes
//! without a registry, each naming where the fault sits.

use nodetool::graph::{self, LoadLocation};

const INNER: &str = "00000000-0000-0000-0000-0000000000f1";
const SINK: &str = "00000000-0000-0000-0000-0000000000b1";

const GROUPED_FILE: &str = "schema_version: 2
name: grouped
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000e1
    type_ref: double stage
    label: The stage
    parameters:
      value: 3
    metadata:
      position: { x: 80, y: 120 }
  - uuid: 00000000-0000-0000-0000-0000000000b1
    type_ref: gamma/int_sink
edges:
  - from: 00000000-0000-0000-0000-0000000000e1
    from_port: value
    to: 00000000-0000-0000-0000-0000000000b1
    to_port: value
groups:
  - name: double stage
    inputs:
      - name: value
        type_refs: [i32]
        node: 00000000-0000-0000-0000-0000000000f1
        port: value
    outputs:
      - name: value
        type_refs: [i32]
        node: 00000000-0000-0000-0000-0000000000f1
        port: value
    nodes:
      - uuid: 00000000-0000-0000-0000-0000000000f1
        type_ref: gamma/doubler
        label: The doubler
        metadata:
          position: { x: 40, y: 20 }
          layers: [a, b]
    edges: []
";

#[test]
fn a_group_round_trips_verbatim() {
    let definition = graph::load(GROUPED_FILE).unwrap();
    assert_eq!(definition.groups.len(), 1);
    let stage = &definition.groups[0];
    assert_eq!(stage.name, "double stage");
    assert_eq!(stage.inputs[0].name, "value");
    assert_eq!(stage.inputs[0].type_refs, vec!["i32".to_owned()]);
    assert_eq!(stage.inputs[0].node.to_string(), INNER);
    assert_eq!(stage.inputs[0].port, "value");
    assert_eq!(stage.outputs.len(), 1);
    assert_eq!(stage.nodes.len(), 1);
    // Inner metadata, labels included, carried verbatim.
    let inner = &stage.nodes[0];
    assert_eq!(inner.label.as_deref(), Some("The doubler"));
    let metadata = &inner.metadata;
    let position = metadata
        .get(graph::Value::String("position".into()))
        .unwrap();
    assert_eq!(
        position.get(graph::Value::String("x".into())),
        Some(&graph::Value::Number(40.into()))
    );
    let layers = metadata.get(graph::Value::String("layers".into())).unwrap();
    assert_eq!(layers.get(1), Some(&graph::Value::String("b".into())));
    // The instance is an ordinary node instance.
    assert_eq!(definition.nodes[0].type_ref, "double stage");
    assert_eq!(definition.nodes[0].label.as_deref(), Some("The stage"));

    let dumped = graph::dump(&definition);
    assert_eq!(graph::load(&dumped).unwrap(), definition);
    assert!(dumped.contains("groups:"), "dumped:\n{dumped}");
    assert!(dumped.contains("type_refs:"), "dumped:\n{dumped}");
}

#[test]
fn a_document_without_groups_dumps_without_the_section() {
    let definition = graph::load(
        "schema_version: 2\nnodes:\n  - uuid: 00000000-0000-0000-0000-0000000000a1\n    type_ref: t\nedges: []\n",
    )
    .unwrap();
    assert!(definition.groups.is_empty());
    let dumped = graph::dump(&definition);
    assert!(!dumped.contains("groups"), "dumped:\n{dumped}");
}

#[test]
fn a_version_1_document_cannot_carry_groups() {
    let (location, message) =
        match graph::load("schema_version: 1\nnodes: []\nedges: []\ngroups: []\n") {
            Err(error) => (error.location, error.message),
            Ok(_) => panic!("a version 1 document carrying groups loads"),
        };
    assert_eq!(location, LoadLocation::Path("schema_version".to_owned()));
    assert!(
        message.contains("version 1") && message.contains("groups"),
        "{message}"
    );
}

#[test]
fn a_group_name_is_unique_in_the_document() {
    let (location, message) = match graph::load(
        "schema_version: 2\nnodes: []\nedges: []\ngroups:\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes: []\n    edges: []\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes: []\n    edges: []\n",
    ) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("duplicate group names load"),
    };
    assert_eq!(location, LoadLocation::Path("groups[1]".to_owned()));
    assert!(
        message.contains("duplicate group name `stage`"),
        "{message}"
    );
}

#[test]
fn an_inner_edge_lands_only_on_its_own_group() {
    let (location, message) = match graph::load(&format!(
        "schema_version: 2\nnodes:\n  - uuid: {SINK}\n    type_ref: t\nedges: []\ngroups:\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes:\n      - uuid: {INNER}\n        type_ref: t\n    edges:\n      - from: {INNER}\n        from_port: a\n        to: {SINK}\n        to_port: b\n"
    )) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("an inner edge landing outside its group loads"),
    };
    assert_eq!(
        location,
        LoadLocation::Path("groups[0].edges[0]".to_owned())
    );
    assert!(
        message.contains("is not defined in group `stage`") && message.contains(SINK),
        "{message}"
    );
}

#[test]
fn a_binding_names_an_inner_node_the_group_contains() {
    let (location, message) = match graph::load(&format!(
        "schema_version: 2\nnodes: []\nedges: []\ngroups:\n  - name: stage\n    inputs:\n      - name: value\n        type_refs: [i32]\n        node: {SINK}\n        port: value\n    outputs: []\n    nodes:\n      - uuid: {INNER}\n        type_ref: t\n    edges: []\n"
    )) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("a binding outside its group loads"),
    };
    assert_eq!(
        location,
        LoadLocation::Path("groups[0].inputs[0]".to_owned())
    );
    assert!(
        message.contains("binds node")
            && message.contains(SINK)
            && message.contains("does not contain"),
        "{message}"
    );
}

#[test]
fn a_groups_inner_graph_carries_the_ordinary_cross_checks() {
    let yaml = format!(
        "schema_version: 2\nnodes: []\nedges: []\ngroups:\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes:\n      - uuid: {INNER}\n        type_ref: t\n      - uuid: {INNER}\n        type_ref: t\n    edges: []\n"
    );
    let (location, message) = match graph::load(&yaml) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("duplicate inner uuids load"),
    };
    assert_eq!(
        location,
        LoadLocation::Path("groups[0].nodes[1]".to_owned())
    );
    assert!(message.contains("duplicate node uuid"), "{message}");
}

#[test]
fn the_definition_carries_groups_to_the_browser_verbatim() {
    let definition = graph::load(GROUPED_FILE).unwrap();
    let carried = serde_json::to_value(&definition).unwrap();
    assert_eq!(carried["groups"][0]["name"], "double stage");
    assert_eq!(carried["groups"][0]["inputs"][0]["port"], "value");
    assert_eq!(
        carried["groups"][0]["nodes"][0]["metadata"]["position"]["x"],
        40
    );
    assert_eq!(carried["nodes"][0]["type_ref"], "double stage");
}
