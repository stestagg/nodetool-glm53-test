//! The engine: a compiled graph ([`crate::compile`]) instantiated as live,
//! streaming execution.
//!
//! The engine invents neither the graph nor the semantics: it consumes the
//! compiled graph and the authoring API ([`crate::behaviour`]). Per node
//! instance it instantiates the node's declared behaviour and wires the
//! compiled shape: each connection one bounded hand-off, an emission
//! delivered to every connected downstream input, an input fixed by a
//! parameter literal driven as a stream that yields once and completes, an
//! input with neither left unconnected. The engine branches on nothing but
//! the compiled graph's generic shape — never on what a node is called or
//! which type it is — and re-validates nothing: compile time already
//! decided, and a run trusts its compiled graph.
//!
//! Every node's [`drive`] runs as its own task, and a run ends only two
//! ways. Every node complete — the wired consumers included, for they are
//! one more downstream of the same fan-out — so awaiting the run is
//! sufficient to have received every value and the end of every consumed
//! stream. Or the first error — a behaviour's, a consumer's, a run task's
//! panic caught and told where its node is known, or the engine refusing a
//! node whose type declares no behaviour — ends the run, fail-fast: the
//! remaining work stops, and the error names the node instance — its label
//! and uuid — and what went wrong. Mid-run cancellation is engine-internal:
//! in-flight values may still sit in a hand-off when the run ends; the
//! guarantee is that the run ends and the error is the last word, not a
//! frozen instant. The tasks ride the caller's tokio runtime: start a run
//! inside one.
//!
//! A run holds its compiled graph read-only and leaves nothing behind on
//! it: a second run of the same compiled graph, and a run of a recompiled
//! definition, each start clean.
//!
//! A run may also be watched: [`Run::observe`] attaches an optional events
//! observer ([`Observer`]) the engine tells each [`Event`] to as the run
//! unfolds — the run started, each node's start, each value emitted (the
//! node, its port, and the value), each node's completion or failure with
//! what went wrong, and the run finished with its outcome. The observer is
//! diagnostics beside the data path, never a second path for the run's
//! product. The engine hands an event over and moves on: events queue
//! unboundedly and one task delivers them at the observer's own pace, so a
//! slow, stalled, absent, or dead observer trades queue growth or silence,
//! never the run's values, timing, or completion — a callback cannot fail
//! into the engine, it is a sink. One node's status is derivable from the
//! events alone: not yet started, running, completed, or failed with its
//! error; a failed run-finished closes every started node that reported
//! neither — the abandoned work fail-fast leaves — as stopped. There is no
//! separate status mechanism beside the events. With no observer — the
//! default — the run is exactly as it would be without one.

use std::any::Any;
use std::collections::HashMap;
use std::future::{poll_fn, Future};
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::{pin, Pin};
use std::sync::{Arc, Mutex};
use std::task::Poll;

use async_trait::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::{JoinError, JoinSet};
use uuid::Uuid;

use crate::behaviour::{self, drive, handoff, Behaviour, Input, Output, Receiver, Sender};
use crate::compile::CompiledGraph;
use crate::{ConvertFn, Value};

/// A run of a compiled graph: the engine's one entry point.
///
/// Build it with [`Run::new`], attach consumers with [`Run::consume`] and
/// an events observer with [`Run::observe`] while it is still being
/// built — wiring is fixed at start — and run it with [`Run::start`]. The
/// run borrows its compiled graph, mutates nothing, and leaves nothing
/// behind: run it again and it starts clean.
pub struct Run<'g> {
    graph: &'g CompiledGraph,
    consumers: Vec<ConsumerWiring>,
    observer: Option<Arc<dyn Observer>>,
}

/// One attached consumer: the node output it joins, and the future built
/// over the stream's receiver.
struct ConsumerWiring {
    node: Uuid,
    port: &'static str,
    with: Consumer,
}

/// A consumer: built over the stream's receiver — every value the output
/// emits from the run's start, in order, then its end.
type Consumer = Box<dyn FnOnce(Receiver<Value>) -> ConsumerStream + Send>;

/// One output's wired downstreams: a hand-off sender per connection, each
/// with the conversion its connection rides.
type Downstream = Vec<(Sender<Value>, Option<ConvertFn>)>;

type ConsumerStream = Pin<Box<dyn Future<Output = Result<(), behaviour::Error>> + Send>>;

impl<'g> Run<'g> {
    /// A run of the compiled graph, nothing attached yet.
    pub fn new(graph: &'g CompiledGraph) -> Run<'g> {
        Run {
            graph,
            consumers: Vec::new(),
            observer: None,
        }
    }

    /// Subscribe the run's events observer: the engine tells it each event
    /// as the run unfolds — diagnostics beside the data path, never a
    /// second path for the run's product. A run with no observer, the
    /// default, tells nothing and runs exactly as it would without one.
    /// An embedder wanting several listeners composes them behind one
    /// observer. Subscribing is for building: past [`Run::start`] the
    /// wiring is fixed.
    pub fn observe(&mut self, observer: Arc<dyn Observer>) {
        self.observer = Some(observer);
    }

    /// Attach one more downstream to a node output's fan-out — the one
    /// routing mechanism the run already wires, so there is no separate
    /// results path and nothing that waits for the run to end. `with`
    /// receives the stream's receiver; its error ends the run like any
    /// downstream failure, and the run does not end while the consumer is
    /// still consuming. Attaching is for building: past [`Run::start`] the
    /// wiring is fixed.
    ///
    /// A node or port the graph does not declare is a bug in the program
    /// building the run, the way a port lookup on a live node is a
    /// behaviour bug: it panics naming the miss.
    pub fn consume<F, Fut>(&mut self, node: Uuid, port: &'static str, with: F)
    where
        F: FnOnce(Receiver<Value>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), behaviour::Error>> + Send + 'static,
    {
        let node_type = &self
            .graph
            .nodes
            .get(&node)
            .unwrap_or_else(|| panic!("this graph has no node {node}"))
            .node_type;
        assert!(
            node_type.outputs.iter().any(|output| output.name == port),
            "node {node} declares no output port `{port}`",
        );
        self.consumers.push(ConsumerWiring {
            node,
            port,
            with: Box::new(move |receiver| Box::pin(with(receiver))),
        });
    }

    /// Run the graph: every node's behaviour driven as its own task, values
    /// propagating as they become available. `Ok(())` is every node
    /// complete, wired consumers included — awaiting the run is sufficient
    /// to have received every value and the end of every consumed stream.
    /// `Err` is the first error surfaced, with the remaining work stopped.
    /// A subscribed observer is told each event as the run unfolds; the
    /// telling never waits on it, and the run ends the same way whether it
    /// listens or not.
    pub async fn start(self) -> Result<(), behaviour::Error> {
        let Run {
            graph,
            consumers,
            observer,
        } = self;
        let mut set = JoinSet::new();

        // Nothing runs a behaviour-less node: its type declared no behaviour
        // to build. Said before anything is wired, so the run never starts
        // half-wired — and before the observer is told anything, for a run
        // refused here never started.
        for (uuid, node) in &graph.nodes {
            if node.node_type.behaviour.is_none() {
                return Err(format!(
                    "node {} ({uuid}) instantiates `{}`, which declares no behaviour to run",
                    node.label, node.node_type.type_ref
                )
                .into());
            }
        }

        // The observer rides beside the data path: events are handed to an
        // unbounded queue — never blocking the emission path — and one
        // task delivers them, at the observer's own pace. The run never
        // waits on that task, so a slow or stalled observer grows the
        // queue, a dead one ends the deliveries, and neither touches the
        // run's values, timing, or completion.
        let events = observer.map(|observer| {
            let (events, queue) = unbounded_channel();
            tokio::spawn(deliver(observer, queue));
            events
        });
        if let Some(events) = &events {
            let _ = events.send(Event::RunStarted);
        }

        // Each connection rides one bounded hand-off: the sender the
        // upstream output holds, the receiver the downstream input holds,
        // and the connection's conversion applied as values cross.
        let mut senders: HashMap<(Uuid, &'static str), Downstream> = HashMap::new();
        let mut receivers: HashMap<(Uuid, &'static str), Receiver<Value>> = HashMap::new();
        for connection in &graph.connections {
            let (sender, receiver) = handoff();
            senders
                .entry((connection.from, connection.from_port))
                .or_default()
                .push((
                    sender,
                    connection.conversion.map(|conversion| conversion.convert),
                ));
            receivers.insert((connection.to, connection.to_port), receiver);
        }

        // An attached consumer joins the fan-out: one more sender on the
        // output it attaches to, its receiver the consumer's stream.
        let mut consumer_streams = Vec::new();
        for consumer in consumers {
            let (sender, receiver) = handoff();
            senders
                .entry((consumer.node, consumer.port))
                .or_default()
                .push((sender, None));
            consumer_streams.push((consumer, receiver));
        }

        for (uuid, node) in &graph.nodes {
            let node_type = node.node_type;
            let mut inputs = Vec::with_capacity(node_type.inputs.len());
            for port in node_type.inputs {
                let input = match receivers.remove(&(*uuid, port.name)) {
                    Some(receiver) => Input::new(port.name, receiver),
                    None => match node.parameters.get(port.name) {
                        // A literal fixed input is driven exactly as the
                        // behaviour promised: a stream that yields once and
                        // completes.
                        Some(parameter) => {
                            let (sender, receiver) = handoff();
                            let value = parameter.value.clone();
                            set.spawn(parameter_stream(value, sender));
                            Input::new(port.name, receiver)
                        }
                        None => Input::unconnected(port.name),
                    },
                };
                inputs.push(input);
            }
            let mut outputs = Vec::with_capacity(node_type.outputs.len());
            for port in node_type.outputs {
                let mut output = Output::new(port.name);
                if let Some(events) = &events {
                    // One watcher per output: it names the node and port
                    // and carries the value as the behaviour emitted it.
                    let events = events.clone();
                    let node = Node {
                        uuid: *uuid,
                        label: node.label.clone(),
                    };
                    output.watch_emissions(Box::new(move |port, value| {
                        let _ = events.send(Event::Emitted {
                            node: node.clone(),
                            port,
                            value: value.clone(),
                        });
                    }));
                }
                if let Some(wired) = senders.remove(&(*uuid, port.name)) {
                    for (sender, conversion) in wired {
                        output.connect(sender, conversion);
                    }
                }
                outputs.push(output);
            }
            let behaviour = (node_type.behaviour.expect("checked before wiring"))();
            let label = node.label.clone();
            set.spawn(run_node(
                *uuid,
                label,
                behaviour,
                inputs,
                outputs,
                events.clone(),
            ));
        }

        for (consumer, receiver) in consumer_streams {
            let label = graph.nodes[&consumer.node].label.clone();
            set.spawn(consume_stream(
                consumer.node,
                label,
                consumer.port,
                consumer.with,
                receiver,
            ));
        }

        // The run ends one of its two ways: every task joined, or the first
        // error told. The tasks are the run's own — nobody else cancels
        // them — so only the join loop cancels, and it returns straight
        // after aborting the rest. Either way the observer is told the
        // run's outcome before the run returns, the run's last event, sent
        // after every node's last one.
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    set.abort_all();
                    let report = error.to_string();
                    if let Some(events) = &events {
                        let _ = events.send(Event::RunFinished {
                            outcome: RunOutcome::Failed(report),
                        });
                    }
                    return Err(error);
                }
                Err(join_error) => {
                    set.abort_all();
                    let lost = lost_run_task(join_error);
                    let report = lost.to_string();
                    if let Some(events) = &events {
                        let _ = events.send(Event::RunFinished {
                            outcome: RunOutcome::Failed(report),
                        });
                    }
                    return Err(lost);
                }
            }
        }
        if let Some(events) = &events {
            let _ = events.send(Event::RunFinished {
                outcome: RunOutcome::Complete,
            });
        }
        Ok(())
    }
}

/// One node's execution: its behaviour driven over its live ports, per the
/// stream semantics. Every error it can end on — a behaviour's or a panic
/// caught here, where the node is known — is told the same way: naming the
/// node instance, its label and uuid, and what went wrong. The observer is
/// told the node's transitions — started, then completed or failed with
/// what went wrong — beside the run path, handed over and never waited on.
async fn run_node(
    uuid: Uuid,
    label: String,
    mut behaviour: Box<dyn Behaviour>,
    mut inputs: Vec<Input>,
    mut outputs: Vec<Output>,
    events: Option<UnboundedSender<Event>>,
) -> Result<(), behaviour::Error> {
    if let Some(events) = &events {
        let _ = events.send(Event::NodeStarted {
            node: Node {
                uuid,
                label: label.clone(),
            },
        });
    }
    let outcome = catch_panic(drive(behaviour.as_mut(), &mut inputs, &mut outputs)).await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(payload) => Err(panicked(payload)),
    };
    match outcome {
        Ok(()) => {
            if let Some(events) = &events {
                let _ = events.send(Event::NodeCompleted {
                    node: Node { uuid, label },
                });
            }
            Ok(())
        }
        Err(error) => {
            let report = format!("node {label} ({uuid}): {error}");
            if let Some(events) = &events {
                let _ = events.send(Event::NodeFailed {
                    node: Node { uuid, label },
                    error: error.to_string(),
                });
            }
            Err(report.into())
        }
    }
}

/// One parameter literal's stream: yields its value once, then ends. A
/// receiving node gone is that stream ending, not an error.
async fn parameter_stream(value: Value, sender: Sender<Value>) -> Result<(), behaviour::Error> {
    let _ = sender.send(value).await;
    Ok(())
}

/// Deliver the run's events to its observer, one at a time in the order
/// the run handed them over. This task runs at the observer's own pace:
/// the run never waits on it, so a slow or stalled observer grows the
/// queue and a dead one — a callback that panicked — ends the deliveries,
/// while the run flows on untouched.
async fn deliver(observer: Arc<dyn Observer>, mut events: UnboundedReceiver<Event>) {
    while let Some(event) = events.recv().await {
        observer.observe(event).await;
    }
}

/// One consumer's stream, run to its end. Every error it can end on — its
/// own or a panic caught here — is told naming the attachment it failed on.
async fn consume_stream(
    uuid: Uuid,
    label: String,
    port: &'static str,
    with: Consumer,
    receiver: Receiver<Value>,
) -> Result<(), behaviour::Error> {
    let outcome = catch_panic(with(receiver)).await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(payload) => Err(panicked(payload)),
    };
    outcome.map_err(|error| format!("a consumer of node {label} ({uuid}) `{port}`: {error}").into())
}

/// The error a caught panic becomes, in the task's own words.
fn panicked(payload: Box<dyn Any + Send>) -> behaviour::Error {
    format!("a run task panicked: {}", panic_message(payload)).into()
}

/// The message a panic payload carries, as far as it tells.
fn panic_message(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&'static str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_else(|| "unknown panic payload".to_owned())
}

/// Runs a future, turning a panic into its payload. A run task's panic is
/// reported through the run's one error path — where the task's identity is
/// known — instead of dying as an anonymous join failure.
async fn catch_panic<F: Future>(future: F) -> Result<F::Output, Box<dyn Any + Send>> {
    let mut future = pin!(future);
    poll_fn(
        |cx| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
            Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(payload)),
        },
    )
    .await
}

/// A run task that neither completed nor reported its error: it died
/// abruptly. A panic is told, never swallowed — the run ends the same
/// fail-fast way.
fn lost_run_task(join_error: JoinError) -> behaviour::Error {
    let reason = match join_error.try_into_panic() {
        Ok(payload) => format!("a run task panicked: {}", panic_message(payload)),
        Err(join_error) => format!("a run task was cancelled mid-run: {join_error}"),
    };
    reason.into()
}

/// The run's voice: what an attached observer is told as the run unfolds.
/// One event model, carried by one observer trait — the engine notifies
/// it and neither knows nor cares who listens. Every event names what it
/// is about: the run, or the node instance and port. A node's status is
/// derivable from these events alone; there is no second event or status
/// mechanism beside them.
#[derive(Debug)]
pub enum Event {
    /// The run is beginning: nothing has started yet.
    RunStarted,
    /// A node's behaviour is being driven, starting now.
    NodeStarted { node: Node },
    /// A node emitted a value on an output port — the value as the
    /// behaviour emitted it, before any conversion a connection rides.
    Emitted {
        node: Node,
        port: &'static str,
        value: Value,
    },
    /// A node completed: its outputs ended, its downstream streams with
    /// them.
    NodeCompleted { node: Node },
    /// A node failed, with what went wrong — the same error the run ends
    /// on, in the node's own words.
    NodeFailed { node: Node, error: String },
    /// The run is over: every node complete, or ended by that one error.
    /// A failed run-finished closes every started node that reported
    /// neither completed nor failed — the abandoned work fail-fast
    /// leaves — as stopped.
    RunFinished { outcome: RunOutcome },
}

/// The run's outcome, as run finished tells it. Another way a run ends —
/// a later story's — extends this, never a second event model.
#[derive(Debug)]
pub enum RunOutcome {
    /// Every node completed; every consumed stream ended.
    Complete,
    /// The first error ended the run — the same report [`Run::start`]
    /// returns.
    Failed(String),
}

/// The identity of a node instance, as every per-node event names it: the
/// uuid addressing it in the compiled graph, and the label a run names it
/// by — the type's default when the definition gave none.
#[derive(Clone, Debug)]
pub struct Node {
    pub uuid: Uuid,
    pub label: String,
}

/// The run's optional witness: the engine hands it each event and moves
/// on. One method, returning nothing — an observer is a sink, and cannot
/// fail into the engine; however slow, stalled, absent, or dead it is,
/// the run's values, timing, and completion are untouched. The engine
/// tells one observer; an embedder wanting several composes them behind
/// one.
#[async_trait]
pub trait Observer: Send + Sync {
    async fn observe(&self, event: Event);
}

/// The printing observer core ships: one readable line per event, the
/// node's label first — the uuid added where labels collide — and values
/// rendered where core can render them, identified by type where it
/// cannot. Subscribing it makes a run's timeline visible from a terminal
/// with no user code. Telling is a sink's business: a line that cannot be
/// written is dropped, never failed into the engine.
pub struct PrintingObserver {
    seen: Mutex<HashMap<String, Uuid>>,
}

impl PrintingObserver {
    pub fn new() -> PrintingObserver {
        PrintingObserver {
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// The node's name on the timeline: its label, with the uuid added
    /// once another node claims the same label.
    fn name(&self, seen: &mut HashMap<String, Uuid>, node: &Node) -> String {
        match seen.get(&node.label) {
            Some(&first) if first != node.uuid => format!("{} ({})", node.label, node.uuid),
            _ => {
                seen.insert(node.label.clone(), node.uuid);
                node.label.clone()
            }
        }
    }
}

impl Default for PrintingObserver {
    fn default() -> PrintingObserver {
        PrintingObserver::new()
    }
}

#[async_trait]
impl Observer for PrintingObserver {
    async fn observe(&self, event: Event) {
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let line = match event {
            Event::RunStarted => "run started".to_owned(),
            Event::NodeStarted { node } => format!("{} started", self.name(&mut seen, &node)),
            Event::Emitted { node, port, value } => format!(
                "{} emitted {port}: {}",
                self.name(&mut seen, &node),
                rendered(&value)
            ),
            Event::NodeCompleted { node } => format!("{} completed", self.name(&mut seen, &node)),
            Event::NodeFailed { node, error } => {
                format!("{} failed: {error}", self.name(&mut seen, &node))
            }
            Event::RunFinished { outcome } => match outcome {
                RunOutcome::Complete => "run completed".to_owned(),
                RunOutcome::Failed(error) => format!("run failed: {error}"),
            },
        };
        // The observer is a sink: a line that cannot be written — a
        // terminal gone, a pipe closed — is dropped on the floor, and
        // never fails into the engine.
        let _ = writeln!(std::io::stdout(), "{line}");
    }
}

/// A value's reading on the timeline: the base scalars core ships render
/// their payload, a string quoted; any other type is identified by its
/// declared name — rendering a plugin's custom type is that plugin's
/// business, not core's.
fn rendered(value: &Value) -> String {
    let rendered = value
        .get::<String>()
        .map(|text| format!("{text:?}"))
        .or_else(|| value.get::<bool>().map(|v| v.to_string()))
        .or_else(|| value.get::<i8>().map(|v| v.to_string()))
        .or_else(|| value.get::<i16>().map(|v| v.to_string()))
        .or_else(|| value.get::<i32>().map(|v| v.to_string()))
        .or_else(|| value.get::<i64>().map(|v| v.to_string()))
        .or_else(|| value.get::<u8>().map(|v| v.to_string()))
        .or_else(|| value.get::<u16>().map(|v| v.to_string()))
        .or_else(|| value.get::<u32>().map(|v| v.to_string()))
        .or_else(|| value.get::<u64>().map(|v| v.to_string()))
        .or_else(|| value.get::<f32>().map(|v| v.to_string()))
        .or_else(|| value.get::<f64>().map(|v| v.to_string()));
    rendered.unwrap_or_else(|| match crate::registry::data_type_by_id(value.type_id()) {
        Some(data_type) => format!("a {} value", data_type.name),
        None => format!("a value of id {}", value.type_id()),
    })
}
