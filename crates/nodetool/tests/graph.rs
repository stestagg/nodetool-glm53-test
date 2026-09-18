//! The graph file format: round-trip fidelity, verbatim metadata, and every
//! structural error path. No plugin is linked — loading is structural and
//! plugin-independent.

use nodetool::graph::{self, LoadLocation, ParameterValue, Value};
use nodetool::registry;

const CIRCLE: &str = "b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33";
const SPLIT: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const UPPER_1: &str = "3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b";
const UPPER_2: &str = "9d8c7b6a-5f4e-4d3c-2b1a-0f9e8d7c6b5a";

/// The full shape of the format: a name, labels, parameters of several
/// kinds, metadata, and one output fanning out to two inputs.
const FULL: &str = "schema_version: 1
name: circle to uppercase
nodes:
  - uuid: b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33
    type_ref: shapes/circle
    label: The circle
    parameters:
      radius: 2.5
      enabled: true
    metadata:
      position: { x: 80, y: 120 }
      color: \"#4a90d9\"
  - uuid: 7c9e6679-7425-40de-944b-e07fc1f90ae7
    type_ref: text/split
    parameters:
      text: \"a,b,c\"
      separator: \",\"
    metadata:
      position: { x: 300, y: 120 }
  - uuid: 3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b
    type_ref: text/uppercase
    metadata:
      position: { x: 520, y: 40 }
  - uuid: 9d8c7b6a-5f4e-4d3c-2b1a-0f9e8d7c6b5a
    type_ref: text/uppercase
edges:
  - from: 7c9e6679-7425-40de-944b-e07fc1f90ae7
    from_port: parts
    to: 3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b
    to_port: text
  - from: 7c9e6679-7425-40de-944b-e07fc1f90ae7
    from_port: parts
    to: 9d8c7b6a-5f4e-4d3c-2b1a-0f9e8d7c6b5a
    to_port: text
";

fn load_error(yaml: &str) -> (LoadLocation, String) {
    let error = graph::load(yaml).unwrap_err();
    (error.location, error.message)
}

fn path_error(yaml: &str) -> (String, String) {
    match load_error(yaml) {
        (LoadLocation::Path(path), message) => (path, message),
        (location, message) => panic!("expected a path location, got {location:?}: {message}"),
    }
}

fn graph_with(parameters: &str) -> String {
    format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\n    parameters:\n{parameters}\nedges: []\n"
    )
}

#[test]
fn a_loaded_file_dumps_back_identical() {
    let definition = graph::load(FULL).unwrap();
    assert_eq!(definition.name.as_deref(), Some("circle to uppercase"));
    assert_eq!(definition.nodes.len(), 4);
    assert_eq!(definition.edges.len(), 2);

    let circle = &definition.nodes[0];
    assert_eq!(circle.type_ref, "shapes/circle");
    assert_eq!(circle.label.as_deref(), Some("The circle"));
    assert_eq!(
        circle.parameters.get("radius"),
        Some(&ParameterValue::Float(2.5))
    );
    assert_eq!(
        circle.parameters.get("enabled"),
        Some(&ParameterValue::Bool(true))
    );

    // One output feeding two downstream inputs is the format's fan-out.
    assert_eq!(definition.edges[0].from.to_string(), SPLIT);
    assert_eq!(definition.edges[0].from_port, "parts");
    assert_eq!(definition.edges[1].from.to_string(), SPLIT);
    assert_ne!(definition.edges[0].to, definition.edges[1].to);

    let dumped = graph::dump(&definition);
    assert_eq!(graph::load(&dumped).unwrap(), definition);
    assert!(dumped.contains("schema_version: 2"), "dumped:\n{dumped}");
}

#[test]
fn dump_omits_fields_the_file_left_out() {
    let definition = graph::load(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\nedges: []\n"
    ))
    .unwrap();
    assert_eq!(definition.name, None);
    assert_eq!(definition.nodes[0].label, None);
    let dumped = graph::dump(&definition);
    assert_eq!(graph::load(&dumped).unwrap(), definition);
    for field in ["name", "label", "parameters", "metadata"] {
        assert!(
            !dumped.contains(field),
            "field {field} in dumped:\n{dumped}"
        );
    }
}

#[test]
fn loading_needs_no_registered_node_types() {
    assert_eq!(
        registry::node_types().count(),
        0,
        "this test binary links no plugin"
    );
    let definition = graph::load(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: never/linked\nedges: []\n"
    ))
    .unwrap();
    assert_eq!(definition.nodes[0].type_ref, "never/linked");
}

#[test]
fn parameter_literals_come_back_as_the_four_kinds() {
    let definition = graph::load(&graph_with(
        "      flag: true\n      count: -3\n      ratio: 2.5\n      text: hello\n",
    ))
    .unwrap();
    let parameters = &definition.nodes[0].parameters;
    assert_eq!(parameters.get("flag"), Some(&ParameterValue::Bool(true)));
    assert_eq!(parameters.get("count"), Some(&ParameterValue::Int(-3)));
    assert_eq!(parameters.get("ratio"), Some(&ParameterValue::Float(2.5)));
    assert_eq!(
        parameters.get("text"),
        Some(&ParameterValue::Str("hello".into()))
    );
    assert_eq!(graph::load(&graph::dump(&definition)).unwrap(), definition);
}

#[test]
fn a_nan_parameter_round_trips_as_itself() {
    let definition = graph::load(&graph_with("      ratio: .nan\n")).unwrap();
    assert_eq!(
        definition.nodes[0].parameters.get("ratio"),
        Some(&ParameterValue::Float(f64::NAN))
    );
    assert_eq!(graph::load(&graph::dump(&definition)).unwrap(), definition);
}

#[test]
fn metadata_is_carried_verbatim() {
    let definition = graph::load(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\n    metadata:\n      position: {{ x: 80, y: 120 }}\n      layers:\n        - a\n        - b\n      1: two\nedges: []\n"
    ))
    .unwrap();
    let metadata = &definition.nodes[0].metadata;
    assert_eq!(metadata.len(), 3);
    let position = metadata.get(Value::String("position".into())).unwrap();
    assert_eq!(
        position.get(Value::String("x".into())),
        Some(&Value::Number(80.into()))
    );
    assert_eq!(
        metadata.get(Value::Number(1.into())),
        Some(&Value::String("two".into()))
    );
    assert_eq!(graph::load(&graph::dump(&definition)).unwrap(), definition);
}

#[test]
fn an_empty_graph_is_representable() {
    let definition = graph::load("schema_version: 1\nnodes: []\nedges: []\n").unwrap();
    assert!(definition.nodes.is_empty());
    assert!(definition.edges.is_empty());
}

#[test]
fn malformed_yaml_reports_line_and_column() {
    // The location is the loader's contract; the message wording is
    // serde_yaml's, so only its presence is asserted.
    let (location, message) = load_error("schema_version: 1\nnodes:\n\t- x\n");
    assert_eq!(location, LoadLocation::Position { line: 3, column: 1 });
    assert!(!message.is_empty(), "{message}");
}

#[test]
fn duplicate_yaml_keys_are_errors() {
    let (location, message) =
        load_error("schema_version: 1\nname: a\nname: b\nnodes: []\nedges: []\n");
    assert!(matches!(location, LoadLocation::Position { .. }));
    assert!(message.contains("duplicate"), "{message}");

    let (_, message) = load_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    uuid: {SPLIT}\n    type_ref: t\nedges: []\n"
    ));
    assert!(message.contains("duplicate"), "{message}");
}

#[test]
fn two_nodes_sharing_a_uuid_are_an_error() {
    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\n  - uuid: {CIRCLE}\n    type_ref: text/split\nedges: []\n"
    ));
    assert_eq!(path, "nodes[1]");
    assert!(message.contains("duplicate node uuid"), "{message}");
    assert!(message.contains(CIRCLE), "{message}");
    assert!(message.contains("nodes[0]"), "{message}");
}

#[test]
fn an_edge_must_reference_nodes_in_the_file() {
    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\nedges:\n  - from: {SPLIT}\n    from_port: parts\n    to: {CIRCLE}\n    to_port: radius\n"
    ));
    assert_eq!(path, "edges[0]");
    assert!(message.contains("`from` node"), "{message}");
    assert!(message.contains(SPLIT), "{message}");

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\nedges:\n  - from: {CIRCLE}\n    from_port: shape\n    to: {SPLIT}\n    to_port: text\n"
    ));
    assert_eq!(path, "edges[0]");
    assert!(message.contains("`to` node"), "{message}");
    assert!(message.contains(SPLIT), "{message}");
}

#[test]
fn an_input_takes_at_most_one_upstream() {
    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {SPLIT}\n    type_ref: text/split\n  - uuid: {UPPER_1}\n    type_ref: text/uppercase\n  - uuid: {UPPER_2}\n    type_ref: text/uppercase\nedges:\n  - from: {SPLIT}\n    from_port: parts\n    to: {UPPER_1}\n    to_port: text\n  - from: {UPPER_2}\n    from_port: text\n    to: {UPPER_1}\n    to_port: text\n"
    ));
    assert_eq!(path, "edges[1]");
    assert!(message.contains("more than one connection"), "{message}");
    assert!(message.contains("input `text` of node"), "{message}");
    assert!(message.contains(UPPER_1), "{message}");
    assert!(message.contains("edges[0]"), "{message}");
}

#[test]
fn an_input_carries_a_connection_or_a_parameter_not_both() {
    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {SPLIT}\n    type_ref: text/split\n  - uuid: {UPPER_1}\n    type_ref: text/uppercase\n    parameters:\n      text: hello\nedges:\n  - from: {SPLIT}\n    from_port: parts\n    to: {UPPER_1}\n    to_port: text\n"
    ));
    assert_eq!(path, "edges[0]");
    assert!(
        message.contains("holds a parameter value and receives a connection"),
        "{message}"
    );
    assert!(message.contains(UPPER_1), "{message}");
}

#[test]
fn a_parameter_value_must_be_a_plain_scalar() {
    for value in [":", ": [1, 2]", ": { x: 1 }"] {
        let (path, message) = path_error(&graph_with(&format!("      a{value}\n")));
        assert_eq!(path, "nodes[0].parameters.a");
        assert!(message.contains("plain scalar"), "{message}");
        assert!(
            message.contains("not a") || message.contains("not null"),
            "{message}"
        );
    }
}

#[test]
fn a_parameter_name_must_be_a_string() {
    let (path, message) = path_error(&graph_with("      1: 5\n"));
    assert_eq!(path, "nodes[0].parameters");
    assert!(message.contains("parameter name"), "{message}");
    assert!(message.contains("not a string"), "{message}");
}

#[test]
fn an_integer_parameter_must_fit_a_signed_integer() {
    let (path, message) = path_error(&graph_with("      count: 18446744073709551615\n"));
    assert_eq!(path, "nodes[0].parameters.count");
    assert!(
        message.contains("does not fit a signed 64-bit integer"),
        "{message}"
    );
}

#[test]
fn unknown_fields_are_errors() {
    let (path, message) =
        path_error("schema_version: 1\nname: x\nnodes: []\nedges: []\nnodez: []\n");
    assert_eq!(path, "document.nodez");
    assert!(message.contains("unknown field `nodez`"), "{message}");

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\n    metadat:\n      position: {{ x: 1, y: 2 }}\nedges: []\n"
    ));
    assert_eq!(path, "nodes[0].metadat");
    assert!(message.contains("unknown field `metadat`"), "{message}");

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\nedges:\n  - from: {CIRCLE}\n    from_port: shape\n    to: {CIRCLE}\n    to_port: radius\n    frm: {CIRCLE}\n"
    ));
    assert_eq!(path, "edges[0].frm");
    assert!(message.contains("unknown field `frm`"), "{message}");
}

#[test]
fn an_unsupported_schema_version_is_an_error() {
    let (path, message) = path_error("schema_version: 3\nnodes: []\nedges: []\n");
    assert_eq!(path, "schema_version");
    assert!(
        message.contains("unsupported schema version 3"),
        "{message}"
    );

    let (path, message) = path_error("schema_version: \"1\"\nnodes: []\nedges: []\n");
    assert_eq!(path, "schema_version");
    assert!(message.contains("not a string"), "{message}");

    let (_, message) = path_error("schema_version: -1\nnodes: []\nedges: []\n");
    assert!(message.contains("not -1"), "{message}");

    let (_, message) = path_error("schema_version: 1.5\nnodes: []\nedges: []\n");
    assert!(message.contains("not 1.5"), "{message}");

    let (path, message) = path_error("nodes: []\nedges: []\n");
    assert_eq!(path, "document");
    assert!(
        message.contains("missing field `schema_version`"),
        "{message}"
    );
}

#[test]
fn both_versions_this_reader_understands_load() {
    for version in [1, 2] {
        let definition = graph::load(&format!(
            "schema_version: {version}\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\nedges: []\n"
        ))
        .unwrap_or_else(|error| panic!("version {version} loads: {error}"));
        // What a definition is, is what the reader writes: the current
        // version, whatever the file carried.
        assert_eq!(definition.schema_version, graph::SCHEMA_VERSION);
    }
}

#[test]
fn the_document_must_carry_nodes_and_edges() {
    let (path, message) = path_error("schema_version: 1\nedges: []\n");
    assert_eq!(path, "document");
    assert!(message.contains("missing field `nodes`"), "{message}");

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: shapes/circle\n"
    ));
    assert_eq!(path, "document");
    assert!(message.contains("missing field `edges`"), "{message}");
}

#[test]
fn structural_fields_must_be_the_right_kind() {
    let (path, message) = path_error("- one\n- two\n");
    assert_eq!(path, "document");
    assert!(
        message.contains("must be a mapping, not a sequence"),
        "{message}"
    );

    let (path, message) = path_error("schema_version: 1\nnodes: { a: 1 }\nedges: []\n");
    assert_eq!(path, "nodes");
    assert!(
        message.contains("must be a sequence, not a mapping"),
        "{message}"
    );

    let (path, message) = path_error("schema_version: 1\nnodes: [nope]\nedges: []\n");
    assert_eq!(path, "nodes[0]");
    assert!(
        message.contains("must be a mapping, not a string"),
        "{message}"
    );

    let (path, message) = path_error("schema_version: 1\nnodes: []\nedges: nope\n");
    assert_eq!(path, "edges");
    assert!(
        message.contains("must be a sequence, not a string"),
        "{message}"
    );

    let (path, message) = path_error("schema_version: 1\nnodes: []\nedges: [nope]\n");
    assert_eq!(path, "edges[0]");
    assert!(
        message.contains("must be a mapping, not a string"),
        "{message}"
    );

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: t\n    metadata: [x]\nedges: []\n"
    ));
    assert_eq!(path, "nodes[0].metadata");
    assert!(
        message.contains("must be a mapping, not a sequence"),
        "{message}"
    );

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: t\n    label: 5\nedges: []\n"
    ));
    assert_eq!(path, "nodes[0].label");
    assert!(
        message.contains("must be a string, not an integer"),
        "{message}"
    );

    let (path, message) = path_error("schema_version: 1\nname: 5\nnodes: []\nedges: []\n");
    assert_eq!(path, "name");
    assert!(
        message.contains("must be a string, not an integer"),
        "{message}"
    );
}

#[test]
fn required_fields_must_be_present_and_valid() {
    let (path, message) = path_error("schema_version: 1\nnodes:\n  - type_ref: t\nedges: []\n");
    assert_eq!(path, "nodes[0]");
    assert!(message.contains("missing field `uuid`"), "{message}");

    let (path, message) =
        path_error("schema_version: 1\nnodes:\n  - uuid: nope\n    type_ref: t\nedges: []\n");
    assert_eq!(path, "nodes[0].uuid");
    assert!(message.contains("invalid uuid `nope`"), "{message}");

    let (path, message) = path_error(&format!(
        "schema_version: 1\nnodes:\n  - uuid: {CIRCLE}\n    type_ref: t\nedges:\n  - from: {CIRCLE}\n    from_port: shape\n    to: {CIRCLE}\n"
    ));
    assert_eq!(path, "edges[0]");
    assert!(message.contains("missing field `to_port`"), "{message}");
}
