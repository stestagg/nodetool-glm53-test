//! The engine: whole-graph wiring and the run lifecycle. Fan-out across
//! nodes, a literal held across a graph's arrivals, a connection's
//! conversion applied as values cross, completion only when every node is
//! done, fail-fast, the stop that ends a run beside its natural ends,
//! values consumed as they arrive, a tapped input reading what the
//! behaviour will read, the empty graph, and runs starting clean — every
//! run observed through the run's own consumer attachments, which are just
//! one more downstream.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::sync::{mpsc, Notify};

use nodetool::async_trait;
use nodetool::compile::{self, CompiledGraph};
use nodetool::engine::{Event, Observer, Run, RunOutcome};
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance, ParameterValue};
use nodetool::registry::Registry;
use test_plugin_delta as _;
use test_plugin_gamma as _;
use uuid::Uuid;

const COUNTER: &str = "00000000-0000-0000-0000-0000000000d1";
const PAIRER: &str = "00000000-0000-0000-0000-0000000000d2";
const DOUBLER_1: &str = "00000000-0000-0000-0000-0000000000d3";
const DOUBLER_2: &str = "00000000-0000-0000-0000-0000000000d4";
const FAILER: &str = "00000000-0000-0000-0000-0000000000d5";
const PASSTHROUGH: &str = "00000000-0000-0000-0000-0000000000d6";
const F64_ECHO: &str = "00000000-0000-0000-0000-0000000000d7";
const MIXED_SOURCE: &str = "00000000-0000-0000-0000-0000000000d8";

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
        groups: Vec::new(),
    }
}

/// Compiles and hands the graph back for the whole process: a run may be
/// spawned as its own task, which borrows its graph for `'static`.
fn compiled(nodes: Vec<NodeInstance>, edges: Vec<Edge>) -> &'static CompiledGraph {
    Box::leak(Box::new(
        compile::compile(&definition(nodes, edges), &Registry::collect())
            .graph
            .expect("the definition compiles"),
    ))
}

/// The counter's whole stream, as the doubler emits it: each value doubled.
fn doubled_values() -> Vec<i32> {
    (1..=50).map(|value| value * 2).collect()
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
async fn drained<T>(mut received: mpsc::UnboundedReceiver<T>) -> Vec<T> {
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
            node(DOUBLER_1, "gamma/doubler"),
            node(DOUBLER_2, "gamma/doubler"),
        ],
        vec![
            edge(COUNTER, "out", DOUBLER_1, "value"),
            edge(COUNTER, "out", DOUBLER_2, "value"),
        ],
    );
    let mut run = Run::new(compiled);
    let first = forwarded(&mut run, DOUBLER_1, "value");
    let second = forwarded(&mut run, DOUBLER_2, "value");

    run.start().await.expect("the run completes");

    for received in [first, second] {
        assert_eq!(
            drained(received).await,
            doubled_values(),
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
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    );
    let mut run = Run::new(compiled);
    // The consumer parks after one value: the doubler's backlog is far from
    // drained and the source stalls behind it, so the nodes are still
    // working.
    let (gate, mut received) = gated(&mut run, DOUBLER_1, "value", 1);
    let started = tokio::spawn(run.start());

    assert_eq!(
        received.recv().await,
        Some(2),
        "a value is consumed as it arrives"
    );
    assert!(
        !started.is_finished(),
        "the nodes are not all complete, so the run has not ended"
    );

    gate.notify_one();
    started
        .await
        .expect("the run task ran")
        .expect("the run completes");
    let doubled = doubled_values();
    assert_eq!(
        drained(received).await,
        &doubled[1..],
        "the rest of the stream arrives once the gate opens"
    );
}

#[tokio::test]
async fn the_run_does_not_end_while_a_wired_consumer_holds_an_undelivered_value() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    );
    let mut run = Run::new(compiled);
    // Every node is done — the source exhausted, the doubler through its
    // last emission — but the hand-off the consumer is wired through still
    // holds undelivered values, and the consumer is still working.
    let (gate, mut received) = gated(&mut run, DOUBLER_1, "value", 45);
    let started = tokio::spawn(run.start());

    for expected in 1..=45 {
        assert_eq!(received.recv().await, Some(expected * 2));
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
    let doubled = doubled_values();
    assert_eq!(drained(received).await, &doubled[45..]);
}

#[tokio::test]
async fn the_first_behaviour_error_ends_the_run_naming_the_node() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            labelled(node(FAILER, "delta/failer"), "the guard"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![
            edge(COUNTER, "out", FAILER, "value"),
            edge(COUNTER, "out", DOUBLER_1, "value"),
        ],
    );
    let mut run = Run::new(compiled);
    // Work unrelated to the failure, parked forever on a gate the test
    // never opens: fail-fast ends the run anyway, where waiting for it
    // would hang.
    let (_gate, _parked) = gated(&mut run, DOUBLER_1, "value", 0);
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
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    );
    let mut run = Run::new(compiled);
    let (forward, mut received) = mpsc::unbounded_channel();
    let node: Uuid = DOUBLER_1.parse().unwrap();
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
        Some(2),
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
        message.contains("Doubler"),
        "the node named by its default label: {message}"
    );
    assert!(
        message.contains(DOUBLER_1),
        "the node's uuid named: {message}"
    );
    assert!(
        message.contains("the consumer gave up"),
        "what went wrong told: {message}"
    );
}

#[test]
#[should_panic(expected = "this graph has no node")]
fn consuming_an_unknown_node_panics_naming_the_miss() {
    let compiled = compiled(vec![node(COUNTER, "delta/counter")], vec![]);
    let missing: Uuid = "00000000-0000-0000-0000-00000000ffee".parse().unwrap();
    Run::new(compiled).consume(missing, "out", |_| async { Ok(()) });
}

#[test]
#[should_panic(expected = "declares no output port")]
fn consuming_an_unknown_port_panics_naming_the_miss() {
    let compiled = compiled(vec![node(COUNTER, "delta/counter")], vec![]);
    Run::new(compiled).consume(COUNTER.parse().unwrap(), "nothing", |_| async { Ok(()) });
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
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    );
    let registry = Registry::collect();
    let compiled = compile::compile(&definition, &registry)
        .graph
        .expect("the definition compiles");

    assert_eq!(
        full_stream(&compiled, DOUBLER_1, "value").await,
        doubled_values()
    );
    assert_eq!(
        full_stream(&compiled, DOUBLER_1, "value").await,
        doubled_values(),
        "a second run of the same compiled graph starts clean"
    );

    let recompiled = compile::compile(&definition, &registry)
        .graph
        .expect("the definition compiles");
    assert_eq!(
        full_stream(&recompiled, DOUBLER_1, "value").await,
        doubled_values(),
        "a run of a recompiled definition starts clean"
    );
}

#[tokio::test]
async fn a_connection_riding_a_conversion_delivers_converted_values() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            node(F64_ECHO, "delta/echo_f64"),
        ],
        vec![edge(COUNTER, "out", F64_ECHO, "value")],
    );
    assert!(
        compiled.connections[0].conversion.is_some(),
        "the compile rides the declared i32→f64 conversion"
    );
    let mut run = Run::new(compiled);
    let (forward, received) = mpsc::unbounded_channel();
    let node: Uuid = F64_ECHO.parse().unwrap();
    run.consume(node, "value", move |mut values| async move {
        while let Some(value) = values.recv().await {
            let value = value
                .get::<f64>()
                .copied()
                .expect("the port is declared f64");
            forward
                .send(value)
                .expect("the test reads what it forwarded");
        }
        Ok(())
    });

    run.start().await.expect("the run completes");

    assert_eq!(
        drained(received).await,
        (1..=50).map(f64::from).collect::<Vec<_>>(),
        "the echo received the counter's whole stream, converted as it crossed"
    );
}

#[tokio::test]
async fn a_tap_reads_the_values_its_input_receives_past_the_connections_conversion() {
    // The counter emits i32 and the echo's input is f64: what a tap on
    // that input reads is what the behaviour will read, not what the
    // upstream emitted.
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            node(F64_ECHO, "delta/echo_f64"),
        ],
        vec![edge(COUNTER, "out", F64_ECHO, "value")],
    );
    let mut run = Run::new(compiled);
    let (forward, received) = mpsc::unbounded_channel();
    let node: Uuid = F64_ECHO.parse().unwrap();
    run.tap(node, "value", move |mut values| async move {
        while let Some(value) = values.recv().await {
            let value = value
                .get::<f64>()
                .copied()
                .expect("the input is declared f64");
            forward
                .send(value)
                .expect("the test reads what it forwarded");
        }
        Ok(())
    });

    run.start().await.expect("the run completes");

    assert_eq!(
        drained(received).await,
        (1..=50).map(f64::from).collect::<Vec<_>>(),
        "every value the input received, converted as it crossed"
    );
}

#[tokio::test]
async fn a_tap_on_an_input_nothing_feeds_ends_with_the_run_delivering_nothing() {
    // The degenerate stream the node itself holds: the tap must end with
    // it, or the run would wait forever on a listener nothing can feed.
    let compiled = compiled(vec![node(F64_ECHO, "delta/echo_f64")], vec![]);
    let mut run = Run::new(compiled);
    let (forward, received) = mpsc::unbounded_channel();
    let node: Uuid = F64_ECHO.parse().unwrap();
    run.tap(node, "value", move |mut values| async move {
        while values.recv().await.is_some() {
            forward.send(()).expect("the test reads what it forwarded");
        }
        Ok(())
    });

    run.start().await.expect("the run completes");

    assert!(drained(received).await.is_empty());
}

#[tokio::test]
async fn a_conversion_that_refuses_a_value_ends_the_run_naming_the_node() {
    let compiled = compiled(
        vec![
            node(MIXED_SOURCE, "delta/mixed_source"),
            node(F64_ECHO, "delta/echo_f64"),
        ],
        vec![edge(MIXED_SOURCE, "mixed", F64_ECHO, "value")],
    );
    assert!(
        compiled.connections[0].conversion.is_some(),
        "the compile rides the i32→f64 conversion although the source also declares String"
    );
    let error = Run::new(compiled)
        .start()
        .await
        .expect_err("the refusal is data-dependent, so a compiling graph fails mid-run");
    let message = error.to_string();
    assert!(
        message.contains("Mixed source"),
        "the error names the node's label: {message}"
    );
    assert!(
        message.contains(MIXED_SOURCE),
        "the error names the node's uuid: {message}"
    );
    assert!(
        message.contains("a run task panicked"),
        "the panic is told, never swallowed: {message}"
    );
    assert!(
        message.contains("the conversion declared on output `mixed` does not take this value"),
        "what went wrong told: {message}"
    );
}

/// An observer that forwards every event to the test through an unbounded
/// channel, in delivery order.
struct Forwarded(mpsc::UnboundedSender<Event>);

#[async_trait]
impl Observer for Forwarded {
    async fn observe(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

/// Attaches the forwarding observer and hands back its timeline.
fn observed(run: &mut Run<'_>) -> mpsc::UnboundedReceiver<Event> {
    let (forward, timeline) = mpsc::unbounded_channel();
    run.observe(Arc::new(Forwarded(forward)));
    timeline
}

/// Reads a timeline up to and including run finished — the run's last
/// event, sent last, delivered in order — so the timeline is complete
/// once read.
async fn until_finished(mut timeline: mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(event) = timeline.recv().await {
        let finished = matches!(event, Event::RunFinished { .. });
        events.push(event);
        if finished {
            break;
        }
    }
    events
}

/// The graph story-05's gate lets a user build: the pairer's second
/// input is neither connected nor parameterised, so its gate never
/// opens and the run makes no progress without error.
fn hung_graph() -> &'static CompiledGraph {
    compiled(
        vec![node(COUNTER, "delta/counter"), node(PAIRER, "delta/pairer")],
        vec![edge(COUNTER, "out", PAIRER, "a")],
    )
}

#[tokio::test]
async fn a_stop_ends_a_run_that_makes_no_progress() {
    let compiled = hung_graph();
    let mut run = Run::new(compiled);
    let timeline = observed(&mut run);
    let (stop, stopped) = watch::channel(false);
    run.stop_on(stopped);
    let started = tokio::spawn(run.start());

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        !started.is_finished(),
        "the gate can never open, so the run waits without ending"
    );
    let _ = stop.send(true);

    tokio::time::timeout(Duration::from_secs(1), started)
        .await
        .expect("the stop ends the hung run promptly")
        .expect("the run task ran")
        .expect("a stopped run is no error");
    let events = until_finished(timeline).await;
    assert!(
        matches!(
            events.last(),
            Some(Event::RunFinished {
                outcome: RunOutcome::Stopped
            })
        ),
        "the stopped outcome is the run's last event: {events:?}"
    );
}

#[tokio::test]
async fn a_stop_ends_a_run_mid_stream_and_abandons_what_is_in_flight() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    );
    let mut run = Run::new(compiled);
    let timeline = observed(&mut run);
    let (gate, mut received) = gated(&mut run, DOUBLER_1, "value", 1);
    let (stop, stopped) = watch::channel(false);
    run.stop_on(stopped);
    let started = tokio::spawn(run.start());

    assert_eq!(
        received.recv().await,
        Some(2),
        "a value is consumed as it arrives"
    );
    assert!(
        !started.is_finished(),
        "the stream has far to go, the run is still on"
    );
    let _ = stop.send(true);

    tokio::time::timeout(Duration::from_secs(1), started)
        .await
        .expect("the stop ends the run promptly, the rest of the stream abandoned")
        .expect("the run task ran")
        .expect("a stopped run is no error");
    drop(gate);
    assert!(
        drained(received).await.is_empty(),
        "nothing further is delivered once the run is stopped"
    );
    let events = until_finished(timeline).await;
    assert!(
        matches!(
            events.last(),
            Some(Event::RunFinished {
                outcome: RunOutcome::Stopped
            })
        ),
        "the stopped outcome is the run's last event: {events:?}"
    );
}

#[tokio::test]
async fn a_stop_asked_before_the_run_begins_still_ends_it_as_stopped() {
    let compiled = hung_graph();
    let mut run = Run::new(compiled);
    let timeline = observed(&mut run);
    let (stop, stopped) = watch::channel(false);
    run.stop_on(stopped);
    let _ = stop.send(true);
    let started = tokio::spawn(run.start());

    started
        .await
        .expect("the run task ran")
        .expect("a stop before the run begins is no error");
    let events = until_finished(timeline).await;
    assert!(
        matches!(
            events.last(),
            Some(Event::RunFinished {
                outcome: RunOutcome::Stopped
            })
        ),
        "the stopped outcome is the run's last event: {events:?}"
    );
}

#[tokio::test]
async fn a_stopped_run_leaves_the_compiled_graph_starting_clean() {
    let compiled = compiled(
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    );

    let mut run = Run::new(compiled);
    let (gate, mut received) = gated(&mut run, DOUBLER_1, "value", 1);
    let (stop, stopped) = watch::channel(false);
    run.stop_on(stopped);
    let started = tokio::spawn(run.start());
    assert_eq!(received.recv().await, Some(2));
    let _ = stop.send(true);
    tokio::time::timeout(Duration::from_secs(1), started)
        .await
        .expect("the stop ends the run promptly")
        .expect("the run task ran")
        .expect("a stopped run is no error");
    drop(gate);

    let received = full_stream(compiled, DOUBLER_1, "value").await;
    assert_eq!(
        received,
        doubled_values(),
        "the same compiled graph runs again, whole and clean"
    );
}
