//! The utility nodes, proven one node at a time and through the compiler: the
//! If routing every pairing decision's value to exactly one selected output,
//! the stream semantics' first-value gating and held-value re-routing
//! arriving unchanged, the Format's two modes across the base scalars with
//! the unfed-template hazard pinned, completion by the default rule, and the
//! compile-time union — exact matches riding no conversion, a plugin custom
//! type bridged by its own declared conversion, and the connection neither
//! can bridge failing with the standard error.

use std::time::Duration;

use tokio::sync::mpsc;

use nodetool::behaviour::{drive, handoff, Behaviour, Input, Output};
use nodetool::compile;
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance};
use nodetool::registry::{self, Registry};
use nodetool::scalars;
use nodetool::Value;
use nodetool_utility as _;

// A custom type with no conversion into any base scalar, and a source
// emitting it: the upstream side of the connection the union cannot bridge.
nodetool::data_type! {
    id: nodetool::uuid!("00000000-0000-0000-0000-080000000001"),
    name: "utility-test/thing",
}

nodetool::node_type! {
    type_ref: "utility-test/thing_source",
    label: "Thing source",
    icon: "<svg/>",
    plugin: "utility-test",
    inputs: [],
    outputs: [ value: "utility-test/thing" ],
}

// A String-emitting source, the exact-match upstream side of the compile
// tests.
nodetool::node_type! {
    type_ref: "utility-test/string_source",
    label: "String source",
    icon: "<svg/>",
    plugin: "utility-test",
    inputs: [],
    outputs: [ value: "String" ],
}

// A custom type whose own declared conversion into `String` is the bridge
// the union cannot supply: the sibling of the unbridgeable `thing`.
const WORD: nodetool::Uuid = nodetool::uuid!("00000000-0000-0000-0000-080000000002");

struct Word(String);

fn word_as_string(value: &Value) -> Option<Value> {
    Some(Value::new(scalars::STRING, value.get::<Word>()?.0.clone()))
}

nodetool::data_type! {
    id: WORD,
    name: "utility-test/word",
    conversions: [ nodetool::scalars::STRING => word_as_string ],
}

nodetool::node_type! {
    type_ref: "utility-test/word_source",
    label: "Word source",
    icon: "<svg/>",
    plugin: "utility-test",
    inputs: [],
    outputs: [ value: "utility-test/word" ],
}

const SOURCE: &str = "00000000-0000-0000-0000-0800000000a1";
const ROUTER: &str = "00000000-0000-0000-0000-0800000000a2";
const FORMATTER: &str = "00000000-0000-0000-0000-0800000000a3";

fn node(uuid: &str, type_ref: &str) -> NodeInstance {
    NodeInstance {
        uuid: uuid.parse().unwrap(),
        type_ref: type_ref.to_owned(),
        label: None,
        parameters: Default::default(),
        metadata: Mapping::new(),
    }
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

/// The behaviour of the named utility node, built through the one registration
/// path — the same way an engine builds it.
fn behaviour_of(type_ref: &str) -> Box<dyn Behaviour> {
    registry::node_type(type_ref)
        .expect("the utility crate declares this node type")
        .behaviour
        .expect("the node type is declared through the authoring API")()
}

/// An input wired to a sender the test feeds.
fn fed(name: &'static str) -> (mpsc::Sender<Value>, Input) {
    let (tx, rx) = handoff();
    (tx, Input::new(name, rx))
}

/// An output wired to one receiver the test reads.
fn collected(name: &'static str) -> (Output, mpsc::Receiver<Value>) {
    let (tx, rx) = handoff();
    let mut output = Output::new(name);
    output.connect(tx, None);
    (output, rx)
}

/// Reads everything a collected output carried, to the stream's end.
async fn drained(mut received: mpsc::Receiver<Value>) -> Vec<String> {
    let mut texts = Vec::new();
    while let Some(value) = received.recv().await {
        texts.push(
            value
                .get::<String>()
                .cloned()
                .expect("the collected port is declared String"),
        );
    }
    texts
}

#[tokio::test]
async fn each_pairing_decision_routes_the_held_value_to_exactly_one_selected_output() {
    let (condition_tx, condition) = fed("condition");
    let (value_tx, value) = fed("value");
    let (then_out, then_rx) = collected("then");
    let (else_out, else_rx) = collected("else");
    let mut node = behaviour_of("utility/if");
    let run = tokio::spawn(async move {
        let mut inputs = [condition, value];
        let mut outputs = [then_out, else_out];
        drive(node.as_mut(), &mut inputs, &mut outputs).await
    });

    // A boolean condition held like a parameter literal, then values: every
    // decision — each value's own arrival and the condition's re-route of the
    // held value — routes to the selected output, and the untouched branch
    // receives nothing from any of them.
    condition_tx
        .send(Value::new(scalars::BOOL, true))
        .await
        .expect("the hand-off takes it");
    value_tx
        .send(Value::new(scalars::STRING, "alpha".to_owned()))
        .await
        .expect("the hand-off takes it");
    value_tx
        .send(Value::new(scalars::STRING, "beta".to_owned()))
        .await
        .expect("the hand-off takes it");
    drop(condition_tx);
    drop(value_tx);

    run.await
        .expect("the node task ran to its end")
        .expect("the node completes");
    assert_eq!(
        drained(then_rx).await,
        ["alpha", "alpha", "beta"],
        "each decision routed to `then`: the condition's arrival re-routed the held value"
    );
    assert!(
        drained(else_rx).await.is_empty(),
        "the untouched branch received nothing"
    );
}

#[tokio::test(start_paused = true)]
async fn a_condition_change_re_routes_the_held_value_under_the_new_condition() {
    let (condition_tx, condition) = fed("condition");
    let (value_tx, value) = fed("value");
    let (then_out, then_rx) = collected("then");
    let (else_out, else_rx) = collected("else");
    let mut node = behaviour_of("utility/if");
    let run = tokio::spawn(async move {
        let mut inputs = [condition, value];
        let mut outputs = [then_out, else_out];
        drive(node.as_mut(), &mut inputs, &mut outputs).await
    });

    // The value arrives while the condition holds none: it queues behind the
    // closed gate, and the sleeps below hand each next send to the driver
    // alone, so the interleaving is the test's, not the polling order's.
    value_tx
        .send(Value::new(scalars::STRING, "alpha".to_owned()))
        .await
        .expect("the hand-off takes it");
    tokio::time::sleep(Duration::from_millis(10)).await;

    // The condition's first value opens the gate: the value's own arrival and
    // the condition's each fire a run under the true condition, each routing
    // the value to exactly one output.
    condition_tx
        .send(Value::new(scalars::BOOL, true))
        .await
        .expect("the hand-off takes it");
    tokio::time::sleep(Duration::from_millis(10)).await;

    // The changed condition's arrival re-runs the held value under the new
    // condition: `alpha` reaches `else` too, on both outputs across the run,
    // every decision still delivering to exactly one of them.
    condition_tx
        .send(Value::new(scalars::BOOL, false))
        .await
        .expect("the hand-off takes it");
    drop(condition_tx);
    drop(value_tx);

    run.await
        .expect("the node task ran to its end")
        .expect("the node completes");
    assert_eq!(
        drained(then_rx).await,
        ["alpha", "alpha"],
        "both runs under the true condition routed the value to `then`"
    );
    assert_eq!(
        drained(else_rx).await,
        ["alpha"],
        "the false arrival re-routed the held value to `else`: the value ran on both outputs across the run"
    );
}

#[tokio::test(start_paused = true)]
async fn a_value_arriving_before_the_conditions_first_value_does_not_fire() {
    let (condition_tx, condition) = fed("condition");
    let (value_tx, value) = fed("value");
    let (then_out, mut then_rx) = collected("then");
    let (else_out, mut else_rx) = collected("else");
    let mut node = behaviour_of("utility/if");
    let run = tokio::spawn(async move {
        let mut inputs = [condition, value];
        let mut outputs = [then_out, else_out];
        drive(node.as_mut(), &mut inputs, &mut outputs).await
    });

    value_tx
        .send(Value::new(scalars::STRING, "alpha".to_owned()))
        .await
        .expect("the hand-off takes it");
    // The gate waits for the condition's first value: no run can have fired,
    // so neither output can have carried anything yet.
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(10), then_rx.recv())
            .await
            .is_err(),
        "no run fired before the condition's first value"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(10), else_rx.recv())
            .await
            .is_err(),
        "no run can have fired on the untouched branch either"
    );

    // The condition's first value opens the gate: the queued value's arrival
    // and the condition's own arrival each route the held value.
    condition_tx
        .send(Value::new(scalars::BOOL, true))
        .await
        .expect("the hand-off takes it");
    drop(condition_tx);
    drop(value_tx);

    run.await
        .expect("the node task ran to its end")
        .expect("the node completes");
    assert_eq!(
        drained(then_rx).await,
        ["alpha", "alpha"],
        "the gated value routed once the condition arrived"
    );
    assert!(drained(else_rx).await.is_empty());
}

#[tokio::test]
async fn the_format_substitutes_the_values_string_form_at_the_templates_first_placeholder() {
    let (template_tx, template) = fed("template");
    let (value_tx, value) = fed("value");
    let (text_out, text_rx) = collected("text");
    let mut node = behaviour_of("utility/format");
    let run = tokio::spawn(async move {
        let mut inputs = [template, value];
        let mut outputs = [text_out];
        drive(node.as_mut(), &mut inputs, &mut outputs).await
    });

    // The template held like a parameter literal pairs against every later
    // value; the values span the union's kinds.
    template_tx
        .send(Value::new(scalars::STRING, "count: {}".to_owned()))
        .await
        .expect("the hand-off takes it");
    for value in [
        Value::new(scalars::I32, 1),
        Value::new(scalars::F64, 2.5),
        Value::new(scalars::BOOL, true),
        Value::new(scalars::STRING, "hi".to_owned()),
    ] {
        value_tx.send(value).await.expect("the hand-off takes it");
    }
    drop(template_tx);
    drop(value_tx);

    run.await
        .expect("the node task ran to its end")
        .expect("the node completes");
    assert_eq!(
        drained(text_rx).await,
        [
            "count: 1",
            "count: 1",
            "count: 2.5",
            "count: true",
            "count: hi"
        ],
        "the template's own arrival fires a run pairing the held value, like any arrival"
    );
}

#[tokio::test]
async fn the_format_without_a_template_yields_the_values_plain_string_form() {
    let (template_tx, template) = fed("template");
    let (value_tx, value) = fed("value");
    let (text_out, text_rx) = collected("text");
    let mut node = behaviour_of("utility/format");
    let run = tokio::spawn(async move {
        let mut inputs = [template, value];
        let mut outputs = [text_out];
        drive(node.as_mut(), &mut inputs, &mut outputs).await
    });

    // The empty template — a graph file's `template: ""` parameter — formats
    // each value's plain string form.
    template_tx
        .send(Value::new(scalars::STRING, String::new()))
        .await
        .expect("the hand-off takes it");
    for value in [
        Value::new(scalars::I32, 1),
        Value::new(scalars::F64, 2.5),
        Value::new(scalars::BOOL, true),
        Value::new(scalars::STRING, "hi".to_owned()),
    ] {
        value_tx.send(value).await.expect("the hand-off takes it");
    }
    drop(template_tx);
    drop(value_tx);

    run.await
        .expect("the node task ran to its end")
        .expect("the node completes");
    assert_eq!(
        drained(text_rx).await,
        ["1", "1", "2.5", "true", "hi"],
        "the empty template's own arrival fires a run pairing the held value, like any arrival"
    );
}

#[tokio::test(start_paused = true)]
async fn a_format_whose_template_input_is_never_fed_hangs_the_run() {
    let (value_tx, value) = fed("value");
    let (text_out, mut text_rx) = collected("text");
    let mut node = behaviour_of("utility/format");
    let run = tokio::spawn(async move {
        let mut inputs = [Input::unconnected("template"), value];
        let mut outputs = [text_out];
        drive(node.as_mut(), &mut inputs, &mut outputs).await
    });

    // A graph file's missing `template` parameter leaves the input
    // unconnected: the degenerate stream never delivers, the gate never
    // opens, and the arriving values queue forever — the run hangs, and
    // nothing is emitted. This is the settled gate semantics, pinned here so
    // a driver change cannot turn it into something else silently.
    value_tx
        .send(Value::new(scalars::I32, 1))
        .await
        .expect("the hand-off takes it");
    drop(value_tx);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), run)
            .await
            .is_err(),
        "the run never completes while the template input is unfed"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(10), text_rx.recv())
            .await
            .is_err(),
        "no run fired, so the node emitted nothing"
    );
}

#[test]
fn the_utility_crate_contributes_exactly_its_two_node_types() {
    let mut contributed: Vec<&str> = registry::node_types()
        .filter(|node_type| node_type.plugin == "utility")
        .map(|node_type| node_type.type_ref)
        .collect();
    contributed.sort_unstable();
    assert_eq!(
        contributed,
        ["utility/format", "utility/if"],
        "the utility crate declares exactly the two named nodes: {contributed:?}"
    );
}

#[test]
fn an_exact_union_member_match_compiles_without_a_conversion() {
    let compiled = compile::compile(
        &definition(
            vec![
                node(SOURCE, "utility-test/string_source"),
                node(ROUTER, "utility/if"),
                node(FORMATTER, "utility/format"),
            ],
            vec![
                edge(SOURCE, "value", ROUTER, "value"),
                edge(ROUTER, "then", FORMATTER, "value"),
            ],
        ),
        &Registry::collect(),
    )
    .expect("the definition compiles");
    assert_eq!(compiled.connections.len(), 2);
    let resolved: Vec<&str> = compiled
        .connections
        .iter()
        .map(|connection| {
            assert!(
                connection.conversion.is_none(),
                "an exact union member matches, no conversion rides"
            );
            connection.resolved_type.name
        })
        .collect();
    assert_eq!(
        resolved,
        ["String", "i8"],
        "the first connection matches its exact member; the union-to-union one resolves to the first declared member both sides share"
    );
}

#[test]
fn a_plugin_custom_type_reaches_format_through_its_declared_conversion() {
    let compiled = compile::compile(
        &definition(
            vec![
                node(SOURCE, "utility-test/word_source"),
                node(FORMATTER, "utility/format"),
            ],
            vec![edge(SOURCE, "value", FORMATTER, "value")],
        ),
        &Registry::collect(),
    )
    .expect("the word type's own declared conversion bridges it into the union");
    let connection = &compiled.connections[0];
    assert_eq!(
        connection.resolved_type.name, "String",
        "the conversion targets the union's String member"
    );
    let conversion = connection
        .conversion
        .expect("the connection rides the word type's declared conversion");
    let converted = (conversion.convert)(&Value::new(WORD, Word("alpha".to_owned())))
        .expect("the declared conversion converts the word");
    assert_eq!(
        converted.get::<String>().map(String::as_str),
        Some("alpha"),
        "the bridged value lands as the String the format formats"
    );
}

#[test]
fn a_connection_the_union_and_declared_conversions_cannot_bridge_fails_to_compile() {
    let errors = compile::compile(
        &definition(
            vec![
                node(SOURCE, "utility-test/thing_source"),
                node(ROUTER, "utility/if"),
            ],
            vec![edge(SOURCE, "value", ROUTER, "value")],
        ),
        &Registry::collect(),
    )
    .expect_err("the union cannot bridge the custom type");
    assert_eq!(errors.len(), 1, "expected one error, got: {errors:?}");
    assert!(
        errors[0].contains("utility-test/thing") && errors[0].contains("String"),
        "the error names both ports' types: {}",
        errors[0]
    );
    assert!(
        errors[0].contains("no exact match and no declared conversion bridges them"),
        "the standard error: {}",
        errors[0]
    );
}
