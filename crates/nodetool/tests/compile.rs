//! The compiler: structure validation, type resolution across declared
//! unions, conversion wiring, the literal rules, and the declared choices
//! an instance must hold one option of. The unknown-instance and
//! doubled-input cases are built as in-memory definitions — a file loaded
//! per the graph format can never carry either.

use std::collections::BTreeMap;

use nodetool::compile::{self, CompiledGraph, CompiledParameter};
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance, ParameterValue};
use nodetool::registry::Registry;
use nodetool::scalars;
use test_plugin_alpha as _;
use test_plugin_beta as _;
use test_plugin_gamma as _;
use uuid::Uuid;

fn parse(uuid: &str) -> Uuid {
    uuid.parse().expect("the test carries a valid uuid")
}

const SOURCE: &str = "00000000-0000-0000-0000-0000000000a1";
const SOURCE_16: &str = "00000000-0000-0000-0000-0000000000a2";
const RATIO: &str = "00000000-0000-0000-0000-0000000000a4";
const SINK: &str = "00000000-0000-0000-0000-0000000000b1";
const SINK_F64: &str = "00000000-0000-0000-0000-0000000000b2";
const SINK_TEXT: &str = "00000000-0000-0000-0000-0000000000b3";
const SINK_F64_2: &str = "00000000-0000-0000-0000-0000000000b4";
const SINK_BOOL: &str = "00000000-0000-0000-0000-0000000000b5";
const SINK_I8: &str = "00000000-0000-0000-0000-0000000000b6";
const SINK_U64: &str = "00000000-0000-0000-0000-0000000000b7";
const SINK_F32: &str = "00000000-0000-0000-0000-0000000000b8";
const PASS_1: &str = "00000000-0000-0000-0000-0000000000c1";
const PASS_2: &str = "00000000-0000-0000-0000-0000000000c2";
const DIAL: &str = "00000000-0000-0000-0000-0000000000d1";
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

fn labelled(mut node: NodeInstance, label: &str) -> NodeInstance {
    node.label = Some(label.to_owned());
    node
}

/// No node uuid leaked into a message: the compiler speaks the names a
/// user gave, or the labels of the types they instantiate.
fn names_no_uuid(message: &str) {
    assert!(
        !message.contains("00000000-0000"),
        "a node uuid reached the message text: {message}"
    );
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
        groups: Vec::new(),
    }
}

fn compile_ok(definition: &GraphDefinition) -> CompiledGraph {
    compile::compile(definition, &registry())
        .graph
        .expect("the definition compiles")
}

fn compile_errors(definition: &GraphDefinition) -> Vec<String> {
    let result = compile::compile(definition, &registry());
    assert!(result.graph.is_none(), "the definition compiles");
    result
        .errors
        .iter()
        .map(|problem| problem.message.clone())
        .collect()
}

fn compile_warnings(definition: &GraphDefinition) -> Vec<String> {
    let result = compile::compile(definition, &registry());
    result
        .warnings
        .iter()
        .map(|problem| problem.message.clone())
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
    names_no_uuid(&error);
    assert!(
        error.contains("Int source") && error.contains("`value`"),
        "{error}"
    );
    assert!(
        error.contains("String sink") && error.contains("`text`"),
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
            labelled(node(PASS_1, "gamma/passthrough"), "there"),
            labelled(node(PASS_2, "gamma/passthrough"), "and back"),
        ],
        vec![
            edge(PASS_1, "value", PASS_2, "value"),
            edge(PASS_2, "value", PASS_1, "value"),
        ],
    ));

    let error = single_error(messages);
    names_no_uuid(&error);
    assert_eq!(error, "cycle: there → and back → there");
}

#[test]
fn an_unknown_node_type_is_an_error_naming_the_reference() {
    let messages = compile_errors(&definition(
        vec![labelled(node(SOURCE, "gamma/missing"), "the stray")],
        vec![],
    ));

    let error = single_error(messages);
    names_no_uuid(&error);
    assert!(
        error.contains("the stray") && error.contains("`gamma/missing`"),
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
fn a_duplicate_instance_uuid_is_an_error_naming_both_nodes() {
    let messages = compile_errors(&definition(
        vec![
            node(SINK, "gamma/int_sink"),
            node(SINK, "gamma/float64_sink"),
        ],
        vec![],
    ));

    let error = single_error(messages);
    names_no_uuid(&error);
    assert_eq!(
        error,
        "the nodes `Int sink` and `Float64 sink` claim the same identity"
    );
}

#[test]
fn a_duplicate_uuid_between_nodes_that_read_alike_names_the_identity() {
    // Two unlabelled sinks of one type read the same, so the colliding
    // uuid is the only key the file offers to tell them apart.
    let messages = compile_errors(&definition(
        vec![node(SINK, "gamma/int_sink"), node(SINK, "gamma/int_sink")],
        vec![],
    ));

    assert_eq!(
        single_error(messages),
        format!("the nodes `Int sink` and `Int sink` claim the same identity {SINK}")
    );
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
    names_no_uuid(&error);
    assert!(
        error.contains("Int source") && error.contains("Int16 source"),
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
fn a_parameter_naming_neither_an_input_port_nor_a_choice_is_an_error() {
    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK, "gamma/int_sink"),
            "nope",
            ParameterValue::Int(1),
        )],
        vec![],
    ));

    let error = single_error(messages);
    names_no_uuid(&error);
    assert!(
        error.contains("Int sink") && error.contains("parameter `nope`"),
        "{error}"
    );
}

#[test]
fn a_declared_choice_compiles_to_the_option_the_instance_named() {
    // The choice rides the parameter map under its own name, beside the
    // port-named literals, and the compiled node reads it back as the
    // option — nothing else about it is core's business.
    let compiled = compile_ok(&definition(
        vec![with_parameter(
            node(DIAL, "beta/dial"),
            "mode",
            ParameterValue::Str("down".to_owned()),
        )],
        vec![],
    ));

    let dial = &compiled.nodes[&parse(DIAL)];
    assert_eq!(dial.choice("mode"), "down");
    // Never a port: no connection can land on a setting.
    assert!(dial.node_type.inputs.iter().all(|port| port.name != "mode"));
}

#[test]
fn a_choice_takes_no_part_in_the_hang_gate() {
    // A dial whose choice and one input are set, the other input left
    // bare: the gate warns about the bare input alone. The choice is
    // carriage for no input, so it neither silences the warning nor earns
    // one of its own.
    let dialled = with_parameter(
        with_parameter(
            node(DIAL, "beta/dial"),
            "mode",
            ParameterValue::Str("up".to_owned()),
        ),
        "value",
        ParameterValue::Int(1),
    );
    let warnings = compile_warnings(&definition(vec![dialled], vec![]));

    let warning = single_error(warnings);
    assert!(warning.contains("`offset`"), "{warning}");
    assert!(!warning.contains("mode"), "{warning}");
}

#[test]
fn a_choice_left_unset_is_an_error_naming_the_node_the_choice_and_its_options() {
    let error = single_error(compile_errors(&definition(
        vec![node(DIAL, "beta/dial")],
        vec![],
    )));

    names_no_uuid(&error);
    assert!(error.contains("Dial"), "{error}");
    assert!(
        error.contains("`mode`") && error.contains("holds no value"),
        "{error}"
    );
    assert!(error.contains("up, down"), "{error}");
}

#[test]
fn a_choice_outside_its_options_is_an_error_naming_the_value() {
    for stray in [
        ParameterValue::Str("sideways".to_owned()),
        ParameterValue::Int(1),
    ] {
        let error = single_error(compile_errors(&definition(
            vec![with_parameter(node(DIAL, "beta/dial"), "mode", stray)],
            vec![],
        )));

        names_no_uuid(&error);
        assert!(error.contains("Dial"), "{error}");
        assert!(
            error.contains("`mode`") && error.contains("outside its options"),
            "{error}"
        );
        assert!(error.contains("up, down"), "{error}");
    }
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
                node(SINK_F64_2, "gamma/float64_sink"),
                "value",
                ParameterValue::Float(2.5),
            ),
            with_parameter(
                node(SINK_TEXT, "gamma/string_sink"),
                "text",
                ParameterValue::Str("hi".into()),
            ),
            with_parameter(
                node(SINK_BOOL, "gamma/bool_sink"),
                "flag",
                ParameterValue::Bool(true),
            ),
        ],
        vec![],
    ));

    let integer = parameter_of(&compiled, SINK, "value");
    assert_eq!(integer.resolved_type.name, "i32");
    assert_eq!(integer.value.get::<i32>(), Some(&3));

    let bridged = parameter_of(&compiled, SINK_F64, "value");
    assert_eq!(bridged.resolved_type.name, "f64");
    assert_eq!(bridged.value.get::<f64>(), Some(&3.0));

    let float = parameter_of(&compiled, SINK_F64_2, "value");
    assert_eq!(float.resolved_type.name, "f64");
    assert_eq!(float.value.get::<f64>(), Some(&2.5));

    let text = parameter_of(&compiled, SINK_TEXT, "text");
    assert_eq!(text.resolved_type.name, "String");
    assert_eq!(text.value.get::<String>().map(String::as_str), Some("hi"));

    let boolean = parameter_of(&compiled, SINK_BOOL, "flag");
    assert_eq!(boolean.resolved_type.name, "bool");
    assert_eq!(boolean.value.get::<bool>(), Some(&true));
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
    names_no_uuid(&error);
    assert!(
        error.contains("Int sink") && error.contains("`value`"),
        "{error}"
    );
    assert!(error.contains("literal \"three\""), "{error}");
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
    assert!(error.contains("literal 1099511627776"), "{error}");
    assert!(error.contains("declared types f64"), "{error}");
}

#[test]
fn a_literal_that_does_not_fit_its_exact_match_type_fails() {
    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK_I8, "gamma/small_sink"),
            "value",
            ParameterValue::Int(300),
        )],
        vec![],
    ));

    let error = single_error(messages);
    assert!(
        error.contains("Small sink") && error.contains("literal 300"),
        "{error}"
    );
    assert!(error.contains("declared types i8"), "{error}");

    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK_U64, "gamma/unsigned_sink"),
            "value",
            ParameterValue::Int(-1),
        )],
        vec![],
    ));

    let error = single_error(messages);
    assert!(
        error.contains("literal -1") && error.contains("declared types u64"),
        "{error}"
    );

    let messages = compile_errors(&definition(
        vec![with_parameter(
            node(SINK_F32, "gamma/float32_sink"),
            "value",
            ParameterValue::Float(1e300),
        )],
        vec![],
    ));

    let error = single_error(messages);
    assert!(
        error.contains("Float32 sink") && error.contains("declared types f32"),
        "{error}"
    );
}

#[test]
fn a_literal_on_the_boundary_of_its_exact_match_type_compiles() {
    let compiled = compile_ok(&definition(
        vec![
            with_parameter(
                node(SINK_I8, "gamma/small_sink"),
                "value",
                ParameterValue::Int(127),
            ),
            with_parameter(
                node(SINK_U64, "gamma/unsigned_sink"),
                "value",
                ParameterValue::Int(0),
            ),
            with_parameter(
                node(SINK_F32, "gamma/float32_sink"),
                "value",
                ParameterValue::Float(3.4e38),
            ),
        ],
        vec![],
    ));

    assert_eq!(
        parameter_of(&compiled, SINK_I8, "value").value.get::<i8>(),
        Some(&127)
    );
    assert_eq!(
        parameter_of(&compiled, SINK_U64, "value")
            .value
            .get::<u64>(),
        Some(&0)
    );
    assert_eq!(
        parameter_of(&compiled, SINK_F32, "value")
            .value
            .get::<f32>(),
        Some(&(3.4e38f64 as f32))
    );

    // The lower integer boundary, and i64's maximum — a literal is an i64, so
    // u64's own maximum is out of a literal's reach.
    let compiled = compile_ok(&definition(
        vec![
            with_parameter(
                node(SINK_I8, "gamma/small_sink"),
                "value",
                ParameterValue::Int(-128),
            ),
            with_parameter(
                node(SINK_U64, "gamma/unsigned_sink"),
                "value",
                ParameterValue::Int(i64::MAX),
            ),
        ],
        vec![],
    ));

    assert_eq!(
        parameter_of(&compiled, SINK_I8, "value").value.get::<i8>(),
        Some(&-128)
    );
    assert_eq!(
        parameter_of(&compiled, SINK_U64, "value")
            .value
            .get::<u64>(),
        Some(&(i64::MAX as u64))
    );
}

#[test]
fn one_compile_reports_every_independent_defect() {
    let messages = compile_errors(&definition(
        vec![
            node(SOURCE, "gamma/missing"),
            with_parameter(
                node(SINK, "gamma/int_sink"),
                "value",
                ParameterValue::Str("three".into()),
            ),
            node(PASS_1, "gamma/passthrough"),
            node(PASS_2, "gamma/passthrough"),
            node(SINK_TEXT, "gamma/string_sink"),
        ],
        vec![
            edge(MISSING, "value", PASS_1, "value"),
            edge(PASS_1, "value", PASS_2, "nope"),
            edge(PASS_1, "value", SINK_TEXT, "text"),
            edge(PASS_2, "value", PASS_2, "value"),
        ],
    ));

    assert_eq!(messages.len(), 6, "{messages:?}");
    let joined = messages.join("\n");
    assert!(joined.contains("no linked plugin declares"), "{joined}");
    assert!(joined.contains("not defined in the graph"), "{joined}");
    assert!(joined.contains("has no input port `nope`"), "{joined}");
    assert!(
        joined.contains("no exact match and no declared conversion"),
        "{joined}"
    );
    assert!(joined.contains("cycle:"), "{joined}");
    assert!(joined.contains("literal \"three\""), "{joined}");
}

#[test]
fn an_input_left_unconnected_and_unparameterised_compiles() {
    let compiled = compile_ok(&definition(vec![node(SINK, "gamma/int_sink")], vec![]));

    assert!(compiled.nodes[&SINK.parse().unwrap()].parameters.is_empty());
    assert!(compiled.connections.is_empty());
}

/// A node with an input left starving beside one that is fed or
/// parameterised: the hang gate, compile's first non-fatal warning. The
/// adder's two inputs make the trap trivial to build.
fn hung_adder() -> GraphDefinition {
    definition(
        vec![
            with_parameter(node(ADD, "alpha/add"), "a", ParameterValue::Int(1)),
            node(SOURCE, "gamma/int_source"),
        ],
        vec![],
    )
}

const ADD: &str = "00000000-0000-0000-0000-0000000000d9";

#[test]
fn a_starving_input_beside_a_fed_one_warns_naming_the_node_the_input_and_the_consequence() {
    let messages = compile_warnings(&hung_adder());
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].contains("Add") && messages[0].contains("`b`"),
        "{}",
        messages[0]
    );
    assert!(messages[0].contains("hang"), "{}", messages[0]);
}

#[test]
fn the_hang_gate_is_exactly_a_starving_input_beside_one_that_arrives() {
    // Both inputs parameterised: nothing hangs.
    let add = with_parameter(
        with_parameter(node(ADD, "alpha/add"), "a", ParameterValue::Int(1)),
        "b",
        ParameterValue::Int(2),
    );
    let fed = definition(
        vec![
            with_parameter(node(ADD, "alpha/add"), "a", ParameterValue::Int(1)),
            node(SOURCE, "gamma/int_source"),
        ],
        vec![edge(SOURCE, "value", ADD, "b")],
    );
    assert!(
        compile_warnings(&definition(vec![add], vec![])).is_empty(),
        "no starving input, no warning"
    );
    assert!(
        compile_warnings(&fed).is_empty(),
        "every input arrives, no warning"
    );

    // Both inputs starving: nothing suggests the node should fire at all,
    // and nothing warns.
    let both = definition(vec![node(ADD, "alpha/add")], vec![]);
    assert!(
        compile_warnings(&both).is_empty(),
        "{:?}",
        compile_warnings(&both)
    );
    // A one-input node left unconnected compiles unwarned, as before.
    let single = definition(vec![node(SINK, "gamma/int_sink")], vec![]);
    assert!(compile_warnings(&single).is_empty());
}

#[test]
fn the_hang_gate_changes_no_outcome_and_its_warning_names_its_node() {
    let result = compile::compile(&hung_adder(), &registry());

    // Advisory: the graph compiles as freely as a clean one.
    let compiled = result.graph.expect("a warning refuses nothing");
    assert!(compiled.nodes.contains_key(&parse(ADD)));

    let warning = &result.warnings[0];
    assert_eq!(warning.nodes, vec![parse(ADD)]);
}

#[test]
fn warnings_travel_beside_errors_in_one_result() {
    let result = compile::compile(
        &definition(
            vec![
                node(SOURCE, "gamma/int_source"),
                node(SINK_TEXT, "gamma/string_sink"),
                with_parameter(node(ADD, "alpha/add"), "a", ParameterValue::Int(1)),
            ],
            vec![edge(SOURCE, "value", SINK_TEXT, "text")],
        ),
        &registry(),
    );

    assert!(
        result.graph.is_none(),
        "the unresolvable connection is an error"
    );
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(result.warnings.len(), 1, "{:?}", result.warnings);
    assert!(
        result.warnings[0].message.contains("Add"),
        "{:?}",
        result.warnings
    );
}

#[test]
fn every_problem_names_the_nodes_it_speaks_of() {
    let result = compile::compile(
        &definition(
            vec![
                node(PASS_1, "gamma/passthrough"),
                node(SOURCE, "gamma/int_source"),
                node(SINK_TEXT, "gamma/string_sink"),
                node(MISSING, "gamma/missing"),
            ],
            vec![edge(SOURCE, "value", SINK_TEXT, "text")],
        ),
        &registry(),
    );

    let unknown = result
        .errors
        .iter()
        .find(|problem| problem.message.contains("no linked plugin declares"))
        .expect("the unknown type is an error");
    assert_eq!(unknown.nodes, vec![parse(MISSING)]);

    let unresolvable = result
        .errors
        .iter()
        .find(|problem| problem.message.contains("no exact match"))
        .expect("the unresolvable connection is an error");
    assert_eq!(unresolvable.nodes, vec![parse(SOURCE), parse(SINK_TEXT)]);
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
