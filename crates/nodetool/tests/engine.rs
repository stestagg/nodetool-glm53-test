//! The engine: whole-graph wiring and the run lifecycle. Fan-out across
//! nodes, a literal held across a graph's arrivals, completion only when
//! every node is done, fail-fast, values consumed as they arrive, the empty
//! graph, and runs starting clean — every run observed through the run's
//! own consumer attachments, which are just one more downstream.

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Notify};

use nodetool::compile::{self, CompiledGraph};
use nodetool::engine::Run;
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance, ParameterValue};
use nodetool::registry::Registry;
use test_plugin_delta as _;
use test_plugin_gamma as _;
use uuid::Uuid;

const COUNTER: &str = "00000000-0000-0000-0000-0000000000d1";
const PAIRER: &str = "00000000-0000-0000-0000-0000000000d2";
const ECHO_1: &str = "00000000-0000-0000-0000-0000000000d3";
const ECHO_2: &str = "00000000-0000-0000-0000-0000000000d4";
const FAILER: &str = "00000000-0000-0000-0000-0000000000d5";
const PASSTHROUGH: &str = "00000000-0000-0000-0000-0000000000d6";

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

/// Compiles and hands the graph back for the whole process: a run may be
/// spawned as its own task, which borrows its graph for `'static`.
fn compiled(nodes: Vec<NodeInstance>, edges: Vec<Edge>) -> &'static CompiledGraph {
    Box::leak(Box::new(
        compile::compile(&definition(nodes, edges), &Registry::collect())
            .expect("the definition compiles"),
    ))
}

/// The counter's whole stream.
fn counter_values() -> Vec<i32> {
    (1..=50).collect()
}

/// Attaches a consumer that forwards every value it receives, in order, to
/// the test through an unbounded channel — as they arrive.
fn forwarded(run: &mut Run<'_>, node: &str, port: &'static str) -> mpsc::UnboundedReceiver<i32> {
    let (forward, received) = mpsc::unbounded_channel();
    let node: Uuid = node.parse().unwrap();
    run.consume(node, port, move |mut values| async move {
        while let Some(value) = values.recv().await {
            let value = value
                .get::<i32>()
                .copied()
                .expect("the port is declared i32");
            forward
                .send(value)
                .expect("the test reads what it forwarded");
        }
        Ok(())
    });
    received
}

/// Reads everything left on a forwarded stream.
async fn drained(mut received: mpsc::UnboundedReceiver<i32>) -> Vec<i32> {
    let mut values = Vec::new();
    while let Some(value) = received.recv().await {
        values.push(value);
    }
    values
}

/// Attaches a consumer that forwards the first `taken` values to the test,
/// then parks on the gate; once the gate opens it forwards the rest. The
/// run-lifecycle tests hold the run in a known state this way.
fn gated(
    run: &mut Run<'_>,
    node: &str,
    port: &'static str,
    taken: usize,
) -> (Arc<Notify>, mpsc::UnboundedReceiver<i32>) {
    let gate = Arc::new(Notify::new());
    let held = gate.clone();
    let (forward, received) = mpsc::unbounded_channel();
    let node: Uuid = node.parse().unwrap();
    run.consume(node, port, move |mut values| async move {
        for _ in 0..taken {
            let value = values.recv().await.expect("the stream carries values");
            forward
                .send(
                    value
                        .get::<i32>()
                        .copied()
                        .expect("the port is declared i32"),
                )
                .expect("the test reads what it forwarded");
        }
        held.notified().await;
        while let Some(value) = values.recv().await {
            forward
                .send(
                    value
                        .get::<i32>()
                        .copied()
                        .expect("the port is declared i32"),
                )
                .expect("the test reads what it forwarded");
        }
        Ok(())
    });
    (gate, received)
}

/// Runs the compiled graph to completion with one consumer on the node's
/// output, and returns the values that consumer received.
async fn full_stream(compiled: &CompiledGraph, node: &str, port: &'static str) -> Vec<i32> {
    let mut run = Run::new(compiled);
    let received = forwarded(&mut run, node, port);
    run.start().await.expect("the run completes");
    drained(received).await
}

#[tokio::test]
async fn one_emission_reaches_every_connected_downstream_node() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            node(ECHO_1, "delta/echo"),
            node(ECHO_2, "delta/echo"),
        ],
        vec![
            edge(COUNTER, "out", ECHO_1, "value"),
            edge(COUNTER, "out", ECHO_2, "value"),
        ],
    );
    let mut run = Run::new(compiled);
    let first = forwarded(&mut run, ECHO_1, "value");
    let second = forwarded(&mut run, ECHO_2, "value");

    run.start().await.expect("the run completes");

    for received in [first, second] {
        assert_eq!(
            drained(received).await,
            counter_values(),
            "each downstream node received the whole stream"
        );
    }
}

#[tokio::test]
async fn a_constant_literal_pairs_against_every_arrival_in_a_whole_graph() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            with_parameter(node(PAIRER, "delta/pairer"), "b", ParameterValue::Int(10)),
        ],
        vec![edge(COUNTER, "out", PAIRER, "a")],
    );
    let mut run = Run::new(compiled);
    let sums = forwarded(&mut run, PAIRER, "sum");

    run.start().await.expect("the run completes");

    let sums = drained(sums).await;
    assert_eq!(
        sums.len(),
        51,
        "every arrival fired its run, and the literal's own arrival did too: {sums:?}"
    );
    for expected in 11..=60 {
        assert!(
            sums.contains(&expected),
            "arrival {expected} paired against the held literal, not only the first: {sums:?}"
        );
    }
}

#[tokio::test]
async fn the_run_ends_only_when_every_node_is_complete() {
    let compiled = compiled(
        vec![node(COUNTER, "delta/counter"), node(ECHO_1, "delta/echo")],
        vec![edge(COUNTER, "out", ECHO_1, "value")],
    );
    let mut run = Run::new(compiled);
    // The consumer parks after one value: the echo's backlog is far from
    // drained and the source stalls behind it, so the nodes are still
    // working.
    let (gate, mut received) = gated(&mut run, ECHO_1, "value", 1);
    let started = tokio::spawn(run.start());

    assert_eq!(received.recv().await, Some(1), "the first value arrived");
    assert!(
        !started.is_finished(),
        "the nodes are not all complete, so the run has not ended"
    );

    gate.notify_one();
    started
        .await
        .expect("the run task ran")
        .expect("the run completes");
    assert_eq!(drained(received).await, (2..=50).collect::<Vec<_>>());
}

#[tokio::test]
async fn the_run_does_not_end_while_a_wired_consumer_holds_an_undelivered_value() {
    let compiled = compiled(
        vec![node(COUNTER, "delta/counter"), node(ECHO_1, "delta/echo")],
        vec![edge(COUNTER, "out", ECHO_1, "value")],
    );
    let mut run = Run::new(compiled);
    // Every node is done — the source exhausted, the echo through its last
    // emission — but the hand-off the consumer is wired through still holds
    // undelivered values, and the consumer is still working.
    let (gate, mut received) = gated(&mut run, ECHO_1, "value", 45);
    let started = tokio::spawn(run.start());

    for expected in 1..=45 {
        assert_eq!(received.recv().await, Some(expected));
    }
    assert!(
        !started.is_finished(),
        "the run does not end while its wiring holds undelivered values"
    );

    gate.notify_one();
    started
        .await
        .expect("the run task ran")
        .expect("the run completes");
    assert_eq!(drained(received).await, (46..=50).collect::<Vec<_>>());
}

#[tokio::test]
async fn values_are_consumed_as_they_arrive_while_the_run_is_still_going() {
    let compiled = compiled(
        vec![node(COUNTER, "delta/counter"), node(ECHO_1, "delta/echo")],
        vec![edge(COUNTER, "out", ECHO_1, "value")],
    );
    let mut run = Run::new(compiled);
    let (gate, mut received) = gated(&mut run, ECHO_1, "value", 1);
    let started = tokio::spawn(run.start());

    let first = received.recv().await.expect("a value arrives");
    assert_eq!(first, 1);
    assert!(
        !started.is_finished(),
        "the value arrived while the run was still going"
    );

    gate.notify_one();
    started
        .await
        .expect("the run task ran")
        .expect("the run completes");
    assert_eq!(drained(received).await, (2..=50).collect::<Vec<_>>());
}

#[tokio::test]
async fn the_first_behaviour_error_ends_the_run_naming_the_node() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            labelled(node(FAILER, "delta/failer"), "the guard"),
            node(ECHO_1, "delta/echo"),
        ],
        vec![
            edge(COUNTER, "out", FAILER, "value"),
            edge(COUNTER, "out", ECHO_1, "value"),
        ],
    );
    let mut run = Run::new(compiled);
    // Work unrelated to the failure, parked forever on a gate the test
    // never opens: fail-fast ends the run anyway, where waiting for it
    // would hang.
    let (_gate, _parked) = gated(&mut run, ECHO_1, "value", 0);
    let started = tokio::spawn(run.start());

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(1), started)
        .await
        .expect("fail-fast ends the run although unrelated work is parked forever");
    let error = outcome
        .expect("the run task ran")
        .expect_err("the run failed");
    let message = error.to_string();
    assert!(
        message.contains("the guard"),
        "the error names the node's label: {message}"
    );
    assert!(
        message.contains(FAILER),
        "the error names the node's uuid: {message}"
    );
    assert!(
        message.contains("the failer ran"),
        "the error tells what went wrong: {message}"
    );
}

#[tokio::test]
async fn a_consumer_error_ends_the_run_like_a_downstream_failure() {
    let compiled = compiled(
        vec![node(COUNTER, "delta/counter"), node(ECHO_1, "delta/echo")],
        vec![edge(COUNTER, "out", ECHO_1, "value")],
    );
    let mut run = Run::new(compiled);
    let (forward, mut received) = mpsc::unbounded_channel();
    let node: Uuid = ECHO_1.parse().unwrap();
    run.consume(node, "value", move |mut values| async move {
        let first = values.recv().await.expect("the stream carries values");
        forward
            .send(
                first
                    .get::<i32>()
                    .copied()
                    .expect("the port is declared i32"),
            )
            .expect("the test reads what it forwarded");
        Err("the consumer gave up".into())
    });
    let started = tokio::spawn(run.start());

    assert_eq!(
        received.recv().await,
        Some(1),
        "values flowed to the consumer"
    );

    let error = started
        .await
        .expect("the run task ran")
        .expect_err("the run failed");
    let message = error.to_string();
    assert!(
        message.contains("a consumer of node"),
        "the failure names the attachment: {message}"
    );
    assert!(
        message.contains("Echo"),
        "the node named by its default label: {message}"
    );
    assert!(message.contains(ECHO_1), "the node's uuid named: {message}");
    assert!(
        message.contains("the consumer gave up"),
        "what went wrong told: {message}"
    );
}

#[tokio::test]
async fn a_node_without_behaviour_ends_the_run_naming_it() {
    let compiled = compiled(vec![node(PASSTHROUGH, "gamma/passthrough")], vec![]);
    let error = Run::new(compiled)
        .start()
        .await
        .expect_err("nothing runs a behaviour-less node");
    let message = error.to_string();
    assert!(
        message.contains("gamma/passthrough"),
        "the type reference named: {message}"
    );
    assert!(
        message.contains(PASSTHROUGH),
        "the node's uuid named: {message}"
    );
    assert!(
        message.contains("no behaviour to run"),
        "what went wrong told: {message}"
    );
}

#[tokio::test]
async fn an_output_with_no_downstream_neither_blocks_nor_fails_the_run() {
    let compiled = compiled(vec![node(COUNTER, "delta/counter")], vec![]);
    Run::new(compiled)
        .start()
        .await
        .expect("the source completes and its discarded values cost nothing");
}

#[tokio::test]
async fn an_empty_graph_finishes_immediately() {
    let compiled = compiled(vec![], vec![]);
    Run::new(compiled)
        .start()
        .await
        .expect("nothing to run: the run is done");
}

#[tokio::test]
async fn a_second_run_of_the_same_compiled_graph_and_of_a_recompiled_definition_starts_clean() {
    let definition = definition(
        vec![node(COUNTER, "delta/counter"), node(ECHO_1, "delta/echo")],
        vec![edge(COUNTER, "out", ECHO_1, "value")],
    );
    let registry = Registry::collect();
    let compiled = compile::compile(&definition, &registry).expect("the definition compiles");

    assert_eq!(
        full_stream(&compiled, ECHO_1, "value").await,
        counter_values()
    );
    assert_eq!(
        full_stream(&compiled, ECHO_1, "value").await,
        counter_values(),
        "a second run of the same compiled graph starts clean"
    );

    let recompiled = compile::compile(&definition, &registry).expect("the definition compiles");
    assert_eq!(
        full_stream(&recompiled, ECHO_1, "value").await,
        counter_values(),
        "a run of a recompiled definition starts clean"
    );
}
