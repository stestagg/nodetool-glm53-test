//! The compiler: structure validation, type resolution across declared
//! unions, conversion wiring, and the literal rules. The unknown-instance
//! and doubled-input cases are built as in-memory definitions — a file
//! loaded per the graph format can never carry either.

use std::collections::BTreeMap;

use nodetool::compile::{self, CompiledGraph, CompiledParameter};
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance, ParameterValue};
use nodetool::registry::Registry;
use nodetool::scalars;
use test_plugin_alpha as _;
use test_plugin_beta as _;
use test_plugin_gamma as _;

const SOURCE: &str = "00000000-0000-0000-0000-0000000000a1";
const SOURCE_16: &str = "00000000-0000-0000-0000-0000000000a2";
const RATIO: &str = "00000000-0000-0000-0000-0000000000a4";
const SINK: &str = "00000000-0000-0000-0000-0000000000b1";
const SINK_F64: &str = "00000000-0000-0000-0000-0000000000b2";
const SINK_TEXT: &str = "00000000-0000-0000-0000-0000000000b3";
const PASS_1: &str = "00000000-0000-0000-0000-0000000000c1";
const PASS_2: &str = "00000000-0000-0000-0000-0000000000c2";
const MISSING: &str = "00000000-0000-0000-0000-0000000000ff";

fn registry() -> Registry {
    Registry::collect()
}

fn node(uuid: &str, type_ref: &str) -> NodeInstance {
    NodeInstance {
        uuid: uuid.parse().unwrap(),
        type_ref: type_ref.to_owned(),
        label: None,
        parameters: BTreeMap::new(),
        metadata: Mapping::new(),
    }
}

fn with_parameter(mut node: NodeInstance, name: &str, value: ParameterValue) -> NodeInstance {
    node.parameters.insert(name.to_owned(), value);
    node
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
    Edge {
        from: from.parse().unwrap(),
        from_port: from_port.to_owned(),
        to: to.parse().unwrap(),
        to_port: to_port.to_owned(),
    }
}

fn definition(nodes: Vec<NodeInstance>, edges: Vec<Edge>) -> GraphDefinition {
    GraphDefinition {
        schema_version: 1,
        name: None,
        nodes,
        edges,
    }
}

fn compile_ok(definition: &GraphDefinition) -> CompiledGraph {
    compile::compile(definition, &registry()).expect("the definition compiles")
}

fn compile_errors(definition: &GraphDefinition) -> Vec<String> {
    compile::compile(definition, &registry())
        .unwrap_err()
        .into_iter()
        .map(|error| error.message)
        .collect()
}

fn single_error(messages: Vec<String>) -> String {
    assert_eq!(messages.len(), 1, "expected one error, got: {messages:?}");
    messages.into_iter().next().unwrap()
}

fn parameter_of<'a>(compiled: &'a CompiledGraph, uuid: &str, port: &str) -> &'a CompiledParameter {
    compiled.nodes[&uuid.parse().unwrap()]
        .parameters
        .get(port)
        .expect("the input holds a compiled parameter")
}

#[test]
fn a_definition_compiles_to_the_same_shape() {
    let compiled = compile_ok(&definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(PASS_1, "gamma/passthrough"),
            node(PASS_2, "gamma/passthrough"),
        ],
        vec![
            edge(SOURCE, "value", PASS_1, "value"),
            edge(SOURCE, "value", PASS_2, "value"),
        ],
    ));

    let uuids: Vec<_> = compiled.nodes.keys().map(|uuid| uuid.to_string()).collect();
    assert_eq!(uuids, [SOURCE, PASS_1, PASS_2]);
    assert_eq!(
        compiled.nodes[&PASS_1.parse().unwrap()].node_type.type_ref,
        "gamma/passthrough"
    );

    assert_eq!(compiled.connections.len(), 2);
    for (connection, expected) in compiled.connections.iter().zip([PASS_1, PASS_2]) {
        assert_eq!(connection.from.to_string(), SOURCE);
        assert_eq!(connection.from_port, "value");
        assert_eq!(connection.to.to_string(), expected);
        assert_eq!(connection.to_port, "value");
        assert_eq!(connection.resolved_type.name, "i32");
        assert!(connection.conversion.is_none());
    }
}

#[test]
fn connections_resolve_across_declared_unions() {
    let compiled = compile_ok(&definition(
        vec![
            node(PASS_1, "beta/identity"),
            node(PASS_2, "beta/identity"),
            node(SINK, "gamma/int_sink"),
            node(SINK_F64, "gamma/float64_sink"),
        ],
        vec![
            edge(PASS_1, "value", SINK, "value"),
            edge(PASS_2, "value", SINK_F64, "value"),
        ],
    ));

    assert_eq!(compiled.connections[0].resolved_type.name, "i32");
    assert!(compiled.connections[0].conversion.is_none());
    assert_eq!(compiled.connections[1].resolved_type.name, "f64");
    assert!(compiled.connections[1].conversion.is_none());
}

#[test]
fn declared_conversions_bridge_connections() {
    let cases: Vec<(&str, &str, &str, &str)> = vec![
        ("gamma/int_source", "gamma/float64_sink", "i32→f64", "f64"),
        ("gamma/int16_source", "gamma/int_sink", "i16→i32", "i32"),
        (
            "gamma/float32_source",
            "gamma/float64_sink",
            "f32→f64",
            "f64",
        ),
    ];
    for (source, sink, declared, resolved) in cases {
        let compiled = compile_ok(&definition(
            vec![node(SOURCE, source), node(SINK, sink)],
            vec![edge(SOURCE, "value", SINK, "value")],
        ));
        let connection = &compiled.connections[0];
        assert_eq!(connection.resolved_type.name, resolved, "{declared}");
        assert!(connection.conversion.is_some(), "{declared}");
    }
}

#[test]
fn a_plugin_custom_type_converts_through_its_declared_conversion() {
    let compiled = compile_ok(&definition(
        vec![
            node(RATIO, "gamma/ratio_source"),
            node(SINK_F64, "gamma/float64_sink"),
        ],
        vec![edge(RATIO, "value", SINK_F64, "value")],
    ));

    let connection = &compiled.connections[0];
    assert_eq!(connection.resolved_type.name, "f64");
    assert_eq!(connection.conversion.unwrap().target, scalars::F64);
}

#[test]
fn a_connection_nothing_bridges_fails_naming_both_sides() {
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(SINK_TEXT, "gamma/string_sink"),
        ],
        vec![edge(SOURCE, "value", SINK_TEXT, "text")],
    ));

    let error = single_error(messages);
    assert!(
        error.contains(SOURCE) && error.contains("`value`"),
        "{error}"
    );
    assert!(
        error.contains(SINK_TEXT) && error.contains("`text`"),
        "{error}"
    );
    assert!(error.contains("i32") && error.contains("String"), "{error}");
    assert!(
        error.contains("no exact match and no declared conversion"),
        "{error}"
    );
}

#[test]
fn a_cycle_is_an_error_naming_the_cycle() {
    let messages = compile_errors(&definition(
        vec![
            node(PASS_1, "gamma/passthrough"),
            node(PASS_2, "gamma/passthrough"),
        ],
        vec![
            edge(PASS_1, "value", PASS_2, "value"),
            edge(PASS_2, "value", PASS_1, "value"),
        ],
    ));

    let error = single_error(messages);
    assert!(error.starts_with("cycle:"), "{error}");
    assert!(error.contains(PASS_1) && error.contains(PASS_2), "{error}");
}

#[test]
fn an_unknown_node_type_is_an_error_naming_the_reference() {
    let messages = compile_errors(&definition(vec![node(SOURCE, "gamma/missing")], vec![]));

    let error = single_error(messages);
    assert!(
        error.contains(SOURCE) && error.contains("`gamma/missing`"),
        "{error}"
    );
}

#[test]
fn an_edge_referencing_an_unknown_instance_is_an_error() {
    let messages = compile_errors(&definition(
        vec![node(PASS_1, "gamma/passthrough")],
        vec![
            edge(MISSING, "value", PASS_1, "value"),
            edge(PASS_1, "value", MISSING, "value"),
        ],
    ));

    assert_eq!(messages.len(), 2, "{messages:?}");
    for error in &messages {
        assert!(
            error.contains(MISSING) && error.contains("not defined in the graph"),
            "{error}"
        );
    }
}

#[test]
fn an_edge_to_a_missing_port_is_an_error() {
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(SINK, "gamma/int_sink"),
        ],
        vec![edge(SOURCE, "nope", SINK, "value")],
    ));
    assert!(
        single_error(messages).contains("has no output port `nope`"),
        "expected the output-port error"
    );

    let messages = compile_errors(&definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(SINK, "gamma/int_sink"),
        ],
        vec![edge(SOURCE, "value", SINK, "nope")],
    ));
    assert!(
        single_error(messages).contains("has no input port `nope`"),
        "expected the input-port error"
    );
}

#[test]
fn an_input_with_two_upstreams_is_an_error() {
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(SOURCE_16, "gamma/int16_source"),
            node(SINK, "gamma/int_sink"),
        ],
        vec![
            edge(SOURCE, "value", SINK, "value"),
            edge(SOURCE_16, "value", SINK, "value"),
        ],
    ));

    let error = single_error(messages);
    assert!(
        error.contains("input `value` of node") && error.contains("more than one connection"),
        "{error}"
    );
    assert!(
        error.contains(SOURCE) && error.contains(SOURCE_16),
        "{error}"
    );
}

#[test]
fn an_input_holding_a_parameter_and_a_connection_is_an_error() {
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            with_parameter(
                node(SINK, "gamma/int_sink"),
                "value",
                ParameterValue::Int(1),
            ),
        ],
        vec![edge(SOURCE, "value", SINK, "value")],
    ));

    let error = single_error(messages);
    assert!(
        error.contains("holds a parameter value and receives a connection"),
        "{error}"
    );
}

#[test]
fn a_parameter_not_naming_an_input_port_is_an_error() {
    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK, "gamma/int_sink"),
            "nope",
            ParameterValue::Int(1),
        )],
        vec![],
    ));

    let error = single_error(messages);
    assert!(
        error.contains(SINK) && error.contains("parameter `nope`"),
        "{error}"
    );
}

#[test]
fn literals_ride_the_connection_rules() {
    let compiled = compile_ok(&definition(
        vec![
            with_parameter(
                node(SINK, "gamma/int_sink"),
                "value",
                ParameterValue::Int(3),
            ),
            with_parameter(
                node(SINK_F64, "gamma/float64_sink"),
                "value",
                ParameterValue::Int(3),
            ),
            with_parameter(
                node("00000000-0000-0000-0000-0000000000b4", "gamma/float64_sink"),
                "value",
                ParameterValue::Float(2.5),
            ),
            with_parameter(
                node(SINK_TEXT, "gamma/string_sink"),
                "text",
                ParameterValue::Str("hi".into()),
            ),
            with_parameter(
                node("00000000-0000-0000-0000-0000000000b5", "gamma/bool_sink"),
                "flag",
                ParameterValue::Bool(true),
            ),
        ],
        vec![],
    ));

    let integer = parameter_of(&compiled, SINK, "value");
    assert_eq!(integer.resolved_type.name, "i32");
    assert_eq!(integer.value, ParameterValue::Int(3));

    let bridged = parameter_of(&compiled, SINK_F64, "value");
    assert_eq!(bridged.resolved_type.name, "f64");
    assert_eq!(bridged.value, ParameterValue::Float(3.0));

    let float = parameter_of(&compiled, "00000000-0000-0000-0000-0000000000b4", "value");
    assert_eq!(float.resolved_type.name, "f64");
    assert_eq!(float.value, ParameterValue::Float(2.5));

    let text = parameter_of(&compiled, SINK_TEXT, "text");
    assert_eq!(text.resolved_type.name, "String");
    assert_eq!(text.value, ParameterValue::Str("hi".into()));

    let boolean = parameter_of(&compiled, "00000000-0000-0000-0000-0000000000b5", "flag");
    assert_eq!(boolean.resolved_type.name, "bool");
    assert_eq!(boolean.value, ParameterValue::Bool(true));
}

#[test]
fn a_literal_nothing_bridges_fails_naming_node_input_and_both_types() {
    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK, "gamma/int_sink"),
            "value",
            ParameterValue::Str("three".into()),
        )],
        vec![],
    ));

    let error = single_error(messages);
    assert!(error.contains(SINK) && error.contains("`value`"), "{error}");
    assert!(error.contains("string literal \"three\""), "{error}");
    assert!(error.contains("declared types i32"), "{error}");
}

#[test]
fn a_literal_outside_the_conversion_source_range_fails() {
    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK_F64, "gamma/float64_sink"),
            "value",
            ParameterValue::Int(1 << 40),
        )],
        vec![],
    ));

    let error = single_error(messages);
    assert!(error.contains("integer literal 1099511627776"), "{error}");
    assert!(error.contains("declared types f64"), "{error}");
}

#[test]
fn an_input_left_unconnected_and_unparameterised_compiles() {
    let compiled = compile_ok(&definition(vec![node(SINK, "gamma/int_sink")], vec![]));

    assert!(compiled.nodes[&SINK.parse().unwrap()].parameters.is_empty());
    assert!(compiled.connections.is_empty());
}

#[test]
fn an_empty_definition_compiles() {
    let compiled = compile_ok(&definition(vec![], vec![]));
    assert!(compiled.nodes.is_empty());
    assert!(compiled.connections.is_empty());
    assert!(compiled.name.is_none());
}

#[test]
fn compilation_is_deterministic() {
    let graph = definition(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(PASS_1, "gamma/passthrough"),
        ],
        vec![edge(SOURCE, "value", PASS_1, "value")],
    );

    // Debug covers every field — types and conversions carry function
    // pointers, so structural equality is not derivable; two runs of the
    // same input still render identically.
    let first = format!("{:?}", compile_ok(&graph));
    let second = format!("{:?}", compile_ok(&graph));
    assert_eq!(first, second);
}
