//! The compiler, resolving the fizzbuzz nodes' numeric families: same-type
//! agreement, a declared conversion bridging, the declaration-order
//! tiebreak, conflicting and source-less families, family literals, the
//! compile-time zero-step rejection, and a family output feeding another
//! node's family — whose resolution waits on the upstream's.

use std::collections::BTreeMap;

use nodetool::compile::{self, CompiledGraph};
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance, ParameterValue};
use nodetool::node_type;
use nodetool::registry::Registry;
use nodetool_fizzbuzz as _;

// The upstream fixtures this suite wires the fizzbuzz nodes against: plain
// one-typed sources, declared here so each resolution case is reachable.
// Descriptor-only: the compiler never runs them.
node_type! {
    type_ref: "tests/int8_source",
    label: "Int8 source",
    icon: "<svg/>",
    plugin: "fizzbuzz-tests",
    inputs: [],
    outputs: [ value: "i8" ],
}
node_type! {
    type_ref: "tests/int16_source",
    label: "Int16 source",
    icon: "<svg/>",
    plugin: "fizzbuzz-tests",
    inputs: [],
    outputs: [ value: "i16" ],
}
node_type! {
    type_ref: "tests/int32_source",
    label: "Int32 source",
    icon: "<svg/>",
    plugin: "fizzbuzz-tests",
    inputs: [],
    outputs: [ value: "i32" ],
}
node_type! {
    type_ref: "tests/float64_source",
    label: "Float64 source",
    icon: "<svg/>",
    plugin: "fizzbuzz-tests",
    inputs: [],
    outputs: [ value: "f64" ],
}
node_type! {
    type_ref: "tests/uint64_source",
    label: "Uint64 source",
    icon: "<svg/>",
    plugin: "fizzbuzz-tests",
    inputs: [],
    outputs: [ value: "u64" ],
}
node_type! {
    type_ref: "tests/bool_source",
    label: "Bool source",
    icon: "<svg/>",
    plugin: "fizzbuzz-tests",
    inputs: [],
    outputs: [ value: "bool" ],
}

const COUNTER: &str = "00000000-0000-0000-0000-0000000000f1";
const CONDITION: &str = "00000000-0000-0000-0000-0000000000f2";
const OUTPUT: &str = "00000000-0000-0000-0000-0000000000f3";
const SOURCE_A: &str = "00000000-0000-0000-0000-0000000000f4";
const SOURCE_B: &str = "00000000-0000-0000-0000-0000000000f5";
const SOURCE_C: &str = "00000000-0000-0000-0000-0000000000f6";

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

fn with_parameters(mut node: NodeInstance, parameters: &[(&str, ParameterValue)]) -> NodeInstance {
    for (name, value) in parameters {
        node.parameters.insert(name.to_string(), value.clone());
    }
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
    compile::compile(definition, &registry()).unwrap_err()
}

fn single_error(messages: Vec<String>) -> String {
    assert_eq!(messages.len(), 1, "expected one error, got: {messages:?}");
    messages.into_iter().next().unwrap()
}

fn family_of(compiled: &CompiledGraph, uuid: &str) -> &'static str {
    compiled.nodes[&uuid.parse().unwrap()]
        .families
        .get("numeric")
        .expect("the numeric family resolved")
        .name
}

fn parameter_of<'a>(compiled: &'a CompiledGraph, uuid: &str, port: &str) -> &'a str {
    compiled.nodes[&uuid.parse().unwrap()]
        .parameters
        .get(port)
        .expect("the input holds a compiled parameter")
        .resolved_type
        .name
}

/// A counter whose start, stop, and step are the given literals.
fn counter(
    uuid: &str,
    start: ParameterValue,
    stop: ParameterValue,
    step: ParameterValue,
) -> NodeInstance {
    with_parameters(
        node(uuid, "fizzbuzz/counter"),
        &[("start", start), ("stop", stop), ("step", step)],
    )
}

#[test]
fn a_family_of_agreeing_connections_resolves_to_their_type() {
    let compiled = compile_ok(&definition(
        vec![
            node(SOURCE_A, "tests/int32_source"),
            node(SOURCE_B, "tests/int32_source"),
            node(CONDITION, "fizzbuzz/eq"),
        ],
        vec![
            edge(SOURCE_A, "value", CONDITION, "a"),
            edge(SOURCE_B, "value", CONDITION, "b"),
        ],
    ));

    assert_eq!(family_of(&compiled, CONDITION), "i32");
    for connection in &compiled.connections {
        assert_eq!(connection.resolved_type.name, "i32");
        assert!(connection.conversion.is_none());
    }
}

#[test]
fn a_declared_conversion_bridges_a_family_source() {
    // i16 and i32 both reach i32 — the i16 connection rides i16→i32.
    let compiled = compile_ok(&definition(
        vec![
            node(SOURCE_A, "tests/int16_source"),
            node(SOURCE_B, "tests/int32_source"),
            node(CONDITION, "fizzbuzz/eq"),
        ],
        vec![
            edge(SOURCE_A, "value", CONDITION, "a"),
            edge(SOURCE_B, "value", CONDITION, "b"),
        ],
    ));

    assert_eq!(family_of(&compiled, CONDITION), "i32");
    let bridged = &compiled.connections[0];
    assert_eq!(bridged.resolved_type.name, "i32");
    assert!(bridged.conversion.is_some());
    let exact = &compiled.connections[1];
    assert_eq!(exact.resolved_type.name, "i32");
    assert!(exact.conversion.is_none());
}

#[test]
fn several_reachable_members_resolve_by_declaration_order() {
    // An i32 source and integer literals reach both i32 and f64; the
    // family's declaration order puts i32 first.
    let compiled = compile_ok(&definition(
        vec![
            node(SOURCE_A, "tests/int32_source"),
            with_parameters(
                node(COUNTER, "fizzbuzz/counter"),
                &[
                    ("stop", ParameterValue::Int(100)),
                    ("step", ParameterValue::Int(1)),
                ],
            ),
        ],
        vec![edge(SOURCE_A, "value", COUNTER, "start")],
    ));

    assert_eq!(family_of(&compiled, COUNTER), "i32");
    assert_eq!(parameter_of(&compiled, COUNTER, "stop"), "i32");
}

#[test]
fn family_literals_pick_the_first_member_every_literal_fits() {
    let compiled = compile_ok(&definition(
        vec![counter(
            COUNTER,
            ParameterValue::Int(1),
            ParameterValue::Int(100),
            ParameterValue::Int(1),
        )],
        vec![],
    ));

    assert_eq!(family_of(&compiled, COUNTER), "i8");
    for port in ["start", "stop", "step"] {
        assert_eq!(parameter_of(&compiled, COUNTER, port), "i8");
    }
    let stop = &compiled.nodes[&COUNTER.parse().unwrap()].parameters["stop"];
    assert_eq!(stop.value.get::<i8>(), Some(&100));

    // 300 does not fit i8, so the family moves to the first member it and
    // every other literal fits.
    let compiled = compile_ok(&definition(
        vec![counter(
            COUNTER,
            ParameterValue::Int(1),
            ParameterValue::Int(300),
            ParameterValue::Int(1),
        )],
        vec![],
    ));

    assert_eq!(family_of(&compiled, COUNTER), "i16");
    let stop = &compiled.nodes[&COUNTER.parse().unwrap()].parameters["stop"];
    assert_eq!(stop.value.get::<i16>(), Some(&300));
}

#[test]
fn conflicting_sources_fail_naming_the_node_ports_and_types() {
    // u64 reaches only u64, f64 only f64; no declared member joins them.
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE_A, "tests/uint64_source"),
            node(SOURCE_B, "tests/float64_source"),
            with_parameters(
                node(COUNTER, "fizzbuzz/counter"),
                &[("step", ParameterValue::Int(1))],
            ),
        ],
        vec![
            edge(SOURCE_A, "value", COUNTER, "start"),
            edge(SOURCE_B, "value", COUNTER, "stop"),
        ],
    ));

    let error = single_error(messages);
    assert!(error.contains(COUNTER), "{error}");
    assert!(
        error.contains("`start`") && error.contains("`stop`"),
        "{error}"
    );
    assert!(error.contains("u64") && error.contains("f64"), "{error}");
}

#[test]
fn a_family_with_no_resolution_source_fails() {
    let error = single_error(compile_errors(&definition(
        vec![node(COUNTER, "fizzbuzz/counter")],
        vec![],
    )));

    assert!(error.contains(COUNTER), "{error}");
    assert!(
        error.contains("`start`") && error.contains("`stop`") && error.contains("`step`"),
        "{error}"
    );
}

#[test]
fn a_zero_step_literal_fails_at_compile_time() {
    let error = single_error(compile_errors(&definition(
        vec![counter(
            COUNTER,
            ParameterValue::Int(1),
            ParameterValue::Int(100),
            ParameterValue::Int(0),
        )],
        vec![],
    )));

    assert!(
        error.contains(COUNTER) && error.contains("`step`"),
        "{error}"
    );
    assert!(error.contains("zero"), "{error}");
}

#[test]
fn a_family_output_feeds_another_node_s_family() {
    // The condition's inputs are fed by the counter's family output: its
    // resolution waits on the counter's, then agrees with it.
    let compiled = compile_ok(&definition(
        vec![
            counter(
                COUNTER,
                ParameterValue::Int(1),
                ParameterValue::Int(100),
                ParameterValue::Int(1),
            ),
            node(CONDITION, "fizzbuzz/lt"),
        ],
        vec![
            edge(COUNTER, "count", CONDITION, "a"),
            edge(COUNTER, "count", CONDITION, "b"),
        ],
    ));

    assert_eq!(family_of(&compiled, COUNTER), "i8");
    assert_eq!(family_of(&compiled, CONDITION), "i8");
    for connection in &compiled.connections {
        assert_eq!(connection.resolved_type.name, "i8");
        assert!(connection.conversion.is_none());
    }
}

#[test]
fn a_family_output_feeds_a_downstream_through_the_compiled_connection() {
    // The connection record is rebuilt against the resolved member, so the
    // compiled graph carries what will actually flow: an f64 source and
    // float literals resolve the counter to f64 — no member earlier in the
    // family's declaration is reachable.
    let compiled = compile_ok(&definition(
        vec![
            node(SOURCE_A, "tests/float64_source"),
            with_parameters(
                node(COUNTER, "fizzbuzz/counter"),
                &[
                    ("stop", ParameterValue::Float(5.0)),
                    ("step", ParameterValue::Float(1.0)),
                ],
            ),
            node(CONDITION, "fizzbuzz/ge"),
        ],
        vec![
            edge(SOURCE_A, "value", COUNTER, "start"),
            edge(COUNTER, "count", CONDITION, "a"),
        ],
    ));

    assert_eq!(family_of(&compiled, COUNTER), "f64");
    assert_eq!(family_of(&compiled, CONDITION), "f64");
    let into_counter = &compiled.connections[0];
    assert_eq!(into_counter.resolved_type.name, "f64");
    assert!(into_counter.conversion.is_none());
    let into_condition = &compiled.connections[1];
    assert_eq!(into_condition.resolved_type.name, "f64");
    assert!(into_condition.conversion.is_none());
}

#[test]
fn a_value_nothing_bridges_fails_to_compile_into_the_output() {
    let error = single_error(compile_errors(&definition(
        vec![
            node(SOURCE_C, "tests/bool_source"),
            node(OUTPUT, "fizzbuzz/output"),
        ],
        vec![edge(SOURCE_C, "value", OUTPUT, "text")],
    )));

    assert!(
        error.contains(OUTPUT) && error.contains("`text`"),
        "{error}"
    );
    assert!(
        error.contains("bool") && error.contains("String"),
        "{error}"
    );
}

#[test]
fn a_parameter_and_a_connection_on_one_family_input_is_still_an_error() {
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE_A, "tests/int32_source"),
            counter(
                COUNTER,
                ParameterValue::Int(1),
                ParameterValue::Int(100),
                ParameterValue::Int(1),
            ),
        ],
        vec![edge(SOURCE_A, "value", COUNTER, "start")],
    ));

    assert!(
        messages
            .iter()
            .any(|error| error.contains("holds a parameter value and receives a connection")),
        "{messages:?}"
    );
}

#[test]
fn the_numeric_ports_span_the_family_s_members() {
    // The listing shows the members, not a marker: every numeric port of
    // every fizzbuzz node declares exactly the family's member list.
    let registry = registry();
    for type_ref in [
        "fizzbuzz/counter",
        "fizzbuzz/case",
        "fizzbuzz/divisible",
        "fizzbuzz/eq",
        "fizzbuzz/ne",
        "fizzbuzz/lt",
        "fizzbuzz/le",
        "fizzbuzz/gt",
        "fizzbuzz/ge",
    ] {
        let node_type = registry.node_type(type_ref).unwrap();
        for port in node_type
            .inputs
            .iter()
            .chain(node_type.outputs.iter())
            .filter(|port| port.family.is_some())
        {
            assert_eq!(port.type_refs, NUMERICS, "{type_ref} `{}`", port.name);
        }
    }
}

const NUMERICS: &[&str] = &[
    "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
];
