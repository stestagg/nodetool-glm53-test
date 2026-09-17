//! The engine's events observer: one event model told as the run unfolds,
//! beside the data path. What each event carries, the order they tell a
//! small run in, the failing run's error and its closure of the abandoned
//! work as stopped, and runs that never notice their observer — stalled,
//! stopped mid-run, panicking, or absent: the run's values and completion
//! are untouched, identical to a run with no observer.

use std::collections::BTreeMap;
use std::future::pending;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;

use nodetool::async_trait;
use nodetool::compile::{self, CompiledGraph};
use nodetool::engine::{Event, Observer, Run, RunOutcome};
use nodetool::graph::{Edge, GraphDefinition, Mapping, NodeInstance};
use nodetool::registry::Registry;
use test_plugin_delta as _;
use test_plugin_gamma as _;
use uuid::Uuid;

const COUNTER: &str = "00000000-0000-0000-0000-0000000000d1";
const DOUBLER_1: &str = "00000000-0000-0000-0000-0000000000d3";
const FAILER: &str = "00000000-0000-0000-0000-0000000000d5";

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

fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
    Edge {
        from: from.parse().unwrap(),
        from_port: from_port.to_owned(),
        to: to.parse().unwrap(),
        to_port: to_port.to_owned(),
    }
}

/// Compiles and hands the graph back for the whole process: a run may be
/// spawned as its own task, which borrows its graph for `'static`.
fn compiled(nodes: Vec<NodeInstance>, edges: Vec<Edge>) -> &'static CompiledGraph {
    Box::leak(Box::new(
        compile::compile(
            &GraphDefinition {
                schema_version: 1,
                name: None,
                nodes,
                edges,
            },
            &Registry::collect(),
        )
        .expect("the definition compiles"),
    ))
}

/// The counter's whole stream, as the doubler emits it: each value doubled.
fn doubled_values() -> Vec<i32> {
    (1..=50).map(|value| value * 2).collect()
}

/// The counter feeding one doubler: the small graph the timeline tests run.
fn counter_to_doubler() -> (Vec<NodeInstance>, Vec<Edge>) {
    (
        vec![
            node(COUNTER, "delta/counter"),
            node(DOUBLER_1, "gamma/doubler"),
        ],
        vec![edge(COUNTER, "out", DOUBLER_1, "value")],
    )
}

/// Attaches a consumer that forwards every value it receives, in order, to
/// the test through an unbounded channel — as they arrive.
fn forwarded(run: &mut Run<'_>, node: Uuid, port: &'static str) -> mpsc::UnboundedReceiver<i32> {
    let (forward, received) = mpsc::unbounded_channel();
    run.consume(node, port, move |mut values| async move {
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

/// Runs the compiled graph to completion with one consumer on the node's
/// output, and returns the values that consumer received.
async fn full_stream(compiled: &CompiledGraph, node: &str, port: &'static str) -> Vec<i32> {
    let mut run = Run::new(compiled);
    let received = forwarded(&mut run, node.parse().unwrap(), port);
    run.start().await.expect("the run completes");
    drained(received).await
}

/// An observer that forwards every event to the test through an unbounded
/// channel, in delivery order.
struct Forwarding(mpsc::UnboundedSender<Event>);

#[async_trait]
impl Observer for Forwarding {
    async fn observe(&self, event: Event) {
        self.0.send(event).expect("the test reads what it forwards");
    }
}

/// Runs the compiled graph to completion with a forwarding observer and a
/// consumer on the node's output, and returns every event in delivery
/// order and every value the consumer received. The events are read up to
/// run finished — the run's last event, sent last, delivered in order — so
/// the timeline is complete once it has been read.
async fn observed(
    compiled: &'static CompiledGraph,
    node: &str,
    port: &'static str,
) -> (Vec<Event>, Vec<i32>) {
    let (forward, mut timeline) = mpsc::unbounded_channel();
    let mut run = Run::new(compiled);
    let received = forwarded(&mut run, node.parse().unwrap(), port);
    run.observe(Arc::new(Forwarding(forward)));
    run.start().await.expect("the run completes");
    let mut events = Vec::new();
    while let Some(event) = timeline.recv().await {
        let finished = matches!(event, Event::RunFinished { .. });
        events.push(event);
        if finished {
            break;
        }
    }
    (events, drained(received).await)
}

/// One node's own events, told in order, as the labels a test reads.
fn told(events: &[Event], uuid: Uuid) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::NodeStarted { node } if node.uuid == uuid => Some("started".to_owned()),
            Event::Emitted { node, port, value } if node.uuid == uuid => Some(format!(
                "emitted {port} {}",
                value
                    .get::<i32>()
                    .copied()
                    .expect("the port is declared i32")
            )),
            Event::NodeCompleted { node } if node.uuid == uuid => Some("completed".to_owned()),
            _ => None,
        })
        .collect()
}

/// A node's status, derived from the events alone: a node absent from the
/// map has not started; its start is told as running; a completed or
/// failed transition is told with what it carries; and a failed
/// run-finished closes every still-running node — the abandoned work
/// fail-fast leaves — as stopped. No other event moves a status.
#[derive(Clone, Debug, PartialEq)]
enum Status {
    Running,
    Complete,
    Failed(String),
    Stopped,
}

fn derived_statuses(events: &[Event]) -> BTreeMap<Uuid, Status> {
    let mut statuses = BTreeMap::new();
    for event in events {
        match event {
            Event::NodeStarted { node } => {
                statuses.insert(node.uuid, Status::Running);
            }
            Event::NodeCompleted { node } => {
                statuses.insert(node.uuid, Status::Complete);
            }
            Event::NodeFailed { node, error } => {
                statuses.insert(node.uuid, Status::Failed(error.clone()));
            }
            Event::RunFinished {
                outcome: RunOutcome::Failed(_),
            } => {
                for status in statuses.values_mut() {
                    if *status == Status::Running {
                        *status = Status::Stopped;
                    }
                }
            }
            _ => {}
        }
    }
    statuses
}

#[tokio::test]
async fn the_events_tell_a_small_run_in_order() {
    let (nodes, edges) = counter_to_doubler();
    let (events, values) = observed(compiled(nodes, edges), DOUBLER_1, "value").await;

    assert_eq!(
        values,
        doubled_values(),
        "the consumer's stream is untouched by the observer"
    );
    assert!(
        matches!(events.first(), Some(Event::RunStarted)),
        "run started is the first event told: {events:?}"
    );
    assert!(
        matches!(
            events.last(),
            Some(Event::RunFinished {
                outcome: RunOutcome::Complete
            })
        ),
        "the run's complete end is the last event told: {events:?}"
    );

    let mut counter = vec!["started".to_owned()];
    counter.extend((1..=50).map(|value| format!("emitted out {value}")));
    counter.push("completed".to_owned());
    assert_eq!(
        told(&events, COUNTER.parse().unwrap()),
        counter,
        "the source's own sequence, in order"
    );

    let mut doubler = vec!["started".to_owned()];
    doubler.extend(
        doubled_values()
            .into_iter()
            .map(|value| format!("emitted value {value}")),
    );
    doubler.push("completed".to_owned());
    assert_eq!(
        told(&events, DOUBLER_1.parse().unwrap()),
        doubler,
        "the doubler's own sequence, in order"
    );
}

#[tokio::test]
async fn an_emitted_event_carries_the_value_and_names_the_node_and_port() {
    let (nodes, edges) = counter_to_doubler();
    let (events, _) = observed(compiled(nodes, edges), DOUBLER_1, "value").await;
    let counter: Uuid = COUNTER.parse().unwrap();

    let emitted = events
        .iter()
        .filter(|event| matches!(event, Event::Emitted { node, .. } if node.uuid == counter))
        .collect::<Vec<_>>();
    let Event::Emitted {
        node,
        port: first_port,
        value: first_value,
    } = emitted.first().expect("the source emitted")
    else {
        panic!("the collected events are the source's emissions: {events:?}");
    };
    assert_eq!(node.label, "Counter", "the node named by its label");
    assert_eq!(node.uuid, counter, "the node named by its uuid");
    assert_eq!(*first_port, "out", "the port named");
    assert_eq!(
        first_value.get::<i32>().copied(),
        Some(1),
        "the value carried as emitted"
    );
    let carried = emitted
        .iter()
        .filter_map(|event| match event {
            Event::Emitted { value, .. } => value.get::<i32>().copied(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        carried,
        (1..=50).collect::<Vec<_>>(),
        "every emission told, in emission order"
    );
}

/// Attaches a consumer parked forever on a gate the test never opens: it
/// holds its stream undelivered — the shape fail-fast abandons.
fn parked(run: &mut Run<'_>, node: Uuid, port: &'static str) {
    run.consume(node, port, move |values| async move {
        let _held = values;
        pending::<()>().await;
        Ok(())
    });
}

#[tokio::test]
async fn a_failing_run_tells_the_error_and_closes_the_abandoned_work_as_stopped() {
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
    let (forward, mut timeline) = mpsc::unbounded_channel();
    let mut run = Run::new(compiled);
    // Work unrelated to the failure, parked forever: fail-fast ends the
    // run with it started but undelivered, where waiting for it would
    // hang.
    parked(&mut run, DOUBLER_1.parse().unwrap(), "value");
    run.observe(Arc::new(Forwarding(forward)));
    let error = run.start().await.expect_err("the guard ends the run");

    let mut events = Vec::new();
    while let Some(event) = timeline.recv().await {
        let finished = matches!(event, Event::RunFinished { .. });
        events.push(event);
        if finished {
            break;
        }
    }
    let statuses = derived_statuses(&events);
    let failer: Uuid = FAILER.parse().unwrap();

    let failed_at = events
        .iter()
        .position(|event| matches!(event, Event::NodeFailed { node, .. } if node.uuid == failer))
        .expect("the guard's failure is told");
    assert_eq!(
        statuses.get(&failer),
        Some(&Status::Failed("the failer ran".to_owned())),
        "the failed transition carries what went wrong: {events:?}"
    );
    let Some(Event::RunFinished {
        outcome: RunOutcome::Failed(report),
    }) = events.last()
    else {
        panic!("the run's last event is its failed end: {events:?}");
    };
    assert!(
        failed_at < events.len() - 1,
        "the node's failure is told before the run's end: {events:?}"
    );
    assert_eq!(
        report,
        &error.to_string(),
        "the run-finished error is the report the run itself returns"
    );
    assert!(
        report.contains("the guard") && report.contains(FAILER),
        "the report names the failing node: {report}"
    );
    assert!(
        report.contains("the failer ran"),
        "the report carries what went wrong: {report}"
    );

    assert_eq!(
        statuses.get(&COUNTER.parse().unwrap()),
        Some(&Status::Stopped),
        "the source was started, told no terminal transition, and the failed run-finished closed it as stopped"
    );
    assert_eq!(
        statuses.get(&DOUBLER_1.parse().unwrap()),
        Some(&Status::Stopped),
        "the doubler was started, told no terminal transition, and the failed run-finished closed it as stopped"
    );
}

/// An observer that never comes back from its first event: its delivery
/// stalls forever, and the run must not care.
struct Stalled;

#[async_trait]
impl Observer for Stalled {
    async fn observe(&self, _event: Event) {
        pending::<()>().await;
    }
}

#[tokio::test]
async fn an_observer_that_blocks_forever_leaves_the_run_untouched() {
    let (nodes, edges) = counter_to_doubler();
    let compiled = compiled(nodes, edges);
    let mut run = Run::new(compiled);
    let received = forwarded(&mut run, DOUBLER_1.parse().unwrap(), "value");
    run.observe(Arc::new(Stalled));

    run.start()
        .await
        .expect("the run completes although its observer is stalled forever");
    assert_eq!(
        drained(received).await,
        doubled_values(),
        "the run's values, timing, and completion are as they would be with no observer"
    );
}

/// An observer that stops mid-run: after a few told events it no longer
/// reacts to any of them.
struct Stops(AtomicUsize);

#[async_trait]
impl Observer for Stops {
    async fn observe(&self, _event: Event) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn an_observer_that_stops_mid_run_leaves_the_run_untouched() {
    let (nodes, edges) = counter_to_doubler();
    let compiled = compiled(nodes, edges);
    let mut run = Run::new(compiled);
    let received = forwarded(&mut run, DOUBLER_1.parse().unwrap(), "value");
    run.observe(Arc::new(Stops(AtomicUsize::new(3))));

    run.start()
        .await
        .expect("the run completes although its observer stopped listening");
    assert_eq!(
        drained(received).await,
        doubled_values(),
        "the run's values, timing, and completion are as they would be with no observer"
    );
}

/// An observer whose callback panics: its delivery dies on it, and the
/// run must not notice.
struct Panics;

#[async_trait]
impl Observer for Panics {
    async fn observe(&self, _event: Event) {
        panic!("the observer is broken");
    }
}

#[tokio::test]
async fn an_observer_whose_callback_panics_leaves_the_run_untouched() {
    let (nodes, edges) = counter_to_doubler();
    let compiled = compiled(nodes, edges);
    let mut run = Run::new(compiled);
    let received = forwarded(&mut run, DOUBLER_1.parse().unwrap(), "value");
    run.observe(Arc::new(Panics));

    run.start()
        .await
        .expect("the run completes although its observer's callback panicked");
    assert_eq!(
        drained(received).await,
        doubled_values(),
        "the run's values, timing, and completion are as they would be with no observer"
    );
}

#[tokio::test]
async fn a_run_with_no_observer_produces_identical_outputs_to_a_run_with_one() {
    let (nodes, edges) = counter_to_doubler();
    let compiled = compiled(nodes, edges);
    let bare = full_stream(compiled, DOUBLER_1, "value").await;
    let (_, watched) = observed(compiled, DOUBLER_1, "value").await;
    assert_eq!(
        bare, watched,
        "the same compiled graph, run twice: subscribing an observer changed nothing"
    );
}
