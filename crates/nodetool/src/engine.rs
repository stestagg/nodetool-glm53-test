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
//! Every node's [`drive`] runs as its own task, and a run ends three ways.
//! Every node complete — the wired consumers included, for they are one
//! more downstream of the same fan-out — so awaiting the run is sufficient
//! to have received every value and the end of every consumed stream. Or
//! the first error — a behaviour's, a consumer's, a run task's panic caught
//! and told where its node is known, or the engine refusing a node whose
//! type declares no behaviour — ends the run, fail-fast: the remaining
//! work stops, and the error names the node instance — its label and uuid
//! — and what went wrong. Or a stop asked on the channel attached with
//! [`Run::stop_on`] ends it: the remaining work stops where it stands and
//! in-flight delivery is not promised — the user's stop adopts the
//! fail-fast cancellation posture, the one ending that abandons undelivered
//! values. Mid-run cancellation is engine-internal: in-flight values may
//! still sit in a hand-off when the run ends; the guarantee is that the run
//! ends and the ending is told, not a frozen instant. The tasks ride the
//! caller's tokio runtime: start a run inside one.
//!
//! A run holds its compiled graph read-only and leaves nothing behind on
//! it: a second run of the same compiled graph, and a run of a recompiled
//! definition, each start clean.
//!
//! A run may also be watched: [`Run::observe`] attaches an optional events
//! observer ([`Observer`]) the engine tells each [`Event`] to as the run
//! unfolds — the run started, each node's start, each value emitted (the
//! node, its port, and the value), each node's completion or failure with
//! what went wrong, and the run finished with its outcome, the failure's
//! node carried in the outcome itself. The observer is diagnostics beside
//! the data path, never a second path for the run's product. The engine
//! hands an event over and moves on: events queue unboundedly and one task
//! delivers them at the observer's own pace, so a
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
use tokio::sync::watch;
use tokio::task::{JoinError, JoinSet};
use uuid::Uuid;

use crate::behaviour::{self, drive, handoff, Behaviour, Input, Output, Receiver, Sender};
use crate::compile::CompiledGraph;
use crate::{ConvertFn, Value};

/// A run of a compiled graph: the engine's one entry point.
///
/// Build it with [`Run::new`], attach consumers with [`Run::consume`],
/// input listeners with [`Run::tap`], an events observer with
/// [`Run::observe`], and a stop channel with
/// [`Run::stop_on`] while it is still being built — wiring is fixed at
/// start — and run it with [`Run::start`]. The run borrows its compiled
/// graph, mutates nothing, and leaves nothing behind: run it again and it
/// starts clean.
pub struct Run<'g> {
    graph: &'g CompiledGraph,
    consumers: Vec<ConsumerWiring>,
    taps: Vec<ConsumerWiring>,
    observer: Option<Arc<dyn Observer>>,
    stop: Option<watch::Receiver<bool>>,
}

/// One attached consumer or tap: the node port it joins, and the future
/// built over the stream's receiver.
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
            taps: Vec::new(),
            observer: None,
            stop: None,
        }
    }

    /// Attach the channel a stop is asked on: a `true` sent on it ends the
    /// run as its third ending — the remaining work stops where it stands,
    /// in-flight delivery is not promised, and the observer is told run
    /// finished stopped. A run making no progress, hung on an input that
    /// never fires, is ended by a stop as readily as a busy one. A stop
    /// arriving before [`Run::start`] begins still ends the run as stopped.
    /// Attaching is for building: past [`Run::start`] the wiring is fixed.
    /// A run with no channel attached — the default — ends only its natural
    /// ways; a channel whose sender is gone can never ask again, and the
    /// run ends naturally.
    pub fn stop_on(&mut self, stop: watch::Receiver<bool>) {
        self.stop = Some(stop);
    }

    /// Subscribe the run's events observer. A run with no observer — the
    /// default — tells nothing and runs exactly as it would without one;
    /// an embedder wanting several listeners composes them behind one
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

    /// Listen on a node *input*: every value it receives, in order, as it
    /// receives it — past the conversion its connection rides, the value
    /// the behaviour will read — then the stream's end. The listener joins
    /// whatever feeds the input as one more downstream beside the node
    /// itself, so it rides the same bounded hand-off: a slow listener
    /// stalls the upstream exactly as a slow node does, and the run does
    /// not end while it is still listening. An input nothing feeds hands
    /// its listeners the degenerate stream the node gets: it ends at once,
    /// delivering nothing. `with`'s error ends the run like any downstream
    /// failure. Attaching is for building: past [`Run::start`] the wiring
    /// is fixed.
    ///
    /// A node or port the graph does not declare is a bug in the program
    /// building the run, as in [`Run::consume`]: it panics naming the miss.
    pub fn tap<F, Fut>(&mut self, node: Uuid, port: &'static str, with: F)
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
            node_type.inputs.iter().any(|input| input.name == port),
            "node {node} declares no input port `{port}`",
        );
        self.taps.push(ConsumerWiring {
            node,
            port,
            with: Box::new(move |receiver| Box::pin(with(receiver))),
        });
    }

    /// Run the graph: every node's behaviour driven as its own task, values
    /// propagating as they become available. `Ok(())` is the run ended
    /// without an error — every node complete, wired consumers included, or
    /// a stop ended it; only the complete ending promises that every value
    /// was delivered, a stop abandoning whatever is in flight. The
    /// observer's run-finished outcome names which ending it was. `Err` is
    /// the first error surfaced, with the remaining work stopped. A
    /// subscribed observer is told each event as the run unfolds; the
    /// telling never waits on it, and the run ends the same way whether it
    /// listens or not. A subscribed stream always opens and closes: even a
    /// run refused before it starts is told run started, then run finished
    /// failed with the refusal.
    pub async fn start(self) -> Result<(), behaviour::Error> {
        let Run {
            graph,
            consumers,
            taps,
            observer,
            mut stop,
        } = self;
        let mut set = JoinSet::new();

        // The events ride an unbounded queue to one delivering task: a send
        // never blocks the emission path, and the delivery runs at the
        // observer's own pace — the run never waits on it.
        let events = observer.map(|observer| {
            let (events, queue) = unbounded_channel();
            tokio::spawn(deliver(observer, queue));
            events
        });
        if let Some(events) = &events {
            let _ = events.send(Event::RunStarted);
        }

        // Nothing runs a behaviour-less node: its type declared no behaviour
        // to build. Refused here, before anything is wired, so the run never
        // starts half-wired — and the refusal ends the run as any error
        // does, closing the stream the run started opened.
        for (uuid, node) in &graph.nodes {
            if node.node_type.behaviour.is_none() {
                let error: behaviour::Error = format!(
                    "node {} ({uuid}) instantiates `{}`, which declares no behaviour to run",
                    node.label, node.node_type.type_ref
                )
                .into();
                run_finished(
                    &events,
                    RunOutcome::Failed {
                        error: error.to_string(),
                        node: Some(Node {
                            uuid: *uuid,
                            label: node.label.clone(),
                        }),
                    },
                );
                return Err(error);
            }
        }

        // An attached consumer or tap joins a fan-out: one more sender
        // beside the wiring's own, its receiver the attachment's stream. A
        // consumer joins the output it names; a tap joins whatever feeds
        // the input it names, resolved with that input below.
        let mut consumer_streams = Vec::new();
        let mut listeners: HashMap<(Uuid, &'static str), Vec<Sender<Value>>> = HashMap::new();
        for tap in taps {
            let (sender, receiver) = handoff();
            listeners
                .entry((tap.node, tap.port))
                .or_default()
                .push(sender);
            consumer_streams.push((tap, receiver));
        }

        // Each connection rides one bounded hand-off: the sender the
        // upstream output holds, the receiver the downstream input holds,
        // and the connection's conversion applied as values cross. A tap on
        // the input rides the same conversion, so it reads what the input
        // receives.
        let mut senders: HashMap<(Uuid, &'static str), Downstream> = HashMap::new();
        let mut receivers: HashMap<(Uuid, &'static str), Receiver<Value>> = HashMap::new();
        for connection in &graph.connections {
            let (sender, receiver) = handoff();
            let convert = connection.conversion.map(|conversion| conversion.convert);
            let downstream = senders
                .entry((connection.from, connection.from_port))
                .or_default();
            downstream.push((sender, convert));
            for listener in listeners
                .remove(&(connection.to, connection.to_port))
                .unwrap_or_default()
            {
                downstream.push((listener, convert));
            }
            receivers.insert((connection.to, connection.to_port), receiver);
        }

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
                        // completes — and a tap on it is driven the same
                        // stream, so a literal is a value the input receives
                        // like any other.
                        Some(parameter) => {
                            let (sender, receiver) = handoff();
                            let value = parameter.value.clone();
                            for listener in
                                listeners.remove(&(*uuid, port.name)).unwrap_or_default()
                            {
                                set.spawn(parameter_stream(value.clone(), listener));
                            }
                            set.spawn(parameter_stream(value, sender));
                            Input::new(port.name, receiver)
                        }
                        // Nothing feeds this input, so nothing feeds a tap
                        // on it: dropping the sender ends the listener's
                        // stream at once, the degenerate stream the node
                        // itself holds.
                        None => {
                            listeners.remove(&(*uuid, port.name));
                            Input::unconnected(port.name)
                        }
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
            let behaviour = (node_type.behaviour.expect("checked before wiring"))(node);
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

        // The run ends one of its three ways: every task joined, or the
        // first error told, or a stop asked. The tasks are the run's own —
        // nobody else cancels them — so only the join loop cancels, and it
        // returns straight after aborting the rest. Either way the observer
        // is told the run's outcome before the run returns, the run's last
        // event, sent after every node's last one.
        loop {
            tokio::select! {
                joined = set.join_next() => match joined {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(failure))) => {
                        set.abort_all();
                        let TaskFailure { error, node } = failure;
                        let report = error.to_string();
                        run_finished(&events, RunOutcome::Failed { error: report, node });
                        return Err(error);
                    }
                    Some(Err(join_error)) => {
                        set.abort_all();
                        let lost = lost_run_task(join_error);
                        run_finished(
                            &events,
                            RunOutcome::Failed {
                                error: lost.to_string(),
                                node: None,
                            },
                        );
                        return Err(lost);
                    }
                    None => {
                        run_finished(&events, RunOutcome::Complete);
                        return Ok(());
                    }
                },
                _ = wait_stop(&mut stop) => {
                    set.abort_all();
                    run_finished(&events, RunOutcome::Stopped);
                    return Ok(());
                }
            }
        }
    }
}

/// Await a stop on the attached channel, if any. A channel whose sender is
/// gone can never ask — the run then ends only its natural ways. Cancel-safe:
/// the select rebuilds the wait every loop, and `changed` keeps its
/// last-seen version.
async fn wait_stop(stop: &mut Option<watch::Receiver<bool>>) {
    let Some(receiver) = stop.as_mut() else {
        return std::future::pending().await;
    };
    loop {
        match receiver.changed().await {
            Ok(()) => return,
            Err(_) => std::future::pending().await,
        }
    }
}

/// One node's execution: its behaviour driven over its live ports, per the
/// stream semantics. Every error it can end on — a behaviour's or a panic
/// caught here, where the node is known — is told the same way: naming the
/// node instance, its label and uuid, and what went wrong, the node riding
/// the returned failure beside the report.
async fn run_node(
    uuid: Uuid,
    label: String,
    mut behaviour: Box<dyn Behaviour>,
    mut inputs: Vec<Input>,
    mut outputs: Vec<Output>,
    events: Option<UnboundedSender<Event>>,
) -> Result<(), TaskFailure> {
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
            let node = Node { uuid, label };
            if let Some(events) = &events {
                let _ = events.send(Event::NodeFailed {
                    node: node.clone(),
                    error: error.to_string(),
                });
            }
            Err(TaskFailure {
                error: format!("node {} ({}): {error}", node.label, node.uuid).into(),
                node: Some(node),
            })
        }
    }
}

/// One parameter literal's stream: yields its value once, then ends. A
/// receiving node gone is that stream ending, not an error.
async fn parameter_stream(value: Value, sender: Sender<Value>) -> Result<(), TaskFailure> {
    let _ = sender.send(value).await;
    Ok(())
}

/// Deliver the run's events to its observer, one at a time in the order
/// the run handed them over, at the observer's own pace. The task ends
/// when the run's event queue ends.
async fn deliver(observer: Arc<dyn Observer>, mut events: UnboundedReceiver<Event>) {
    while let Some(event) = events.recv().await {
        observer.observe(event).await;
    }
}

/// The run's last event: its outcome, sent once the run's end is known,
/// after every node's last event.
fn run_finished(events: &Option<UnboundedSender<Event>>, outcome: RunOutcome) {
    if let Some(events) = events {
        let _ = events.send(Event::RunFinished { outcome });
    }
}

/// A run task's failure: the report it ends the run with, and the node
/// the failure belongs to — known where the task was built. The run's
/// end reads both off the task's own return, in one piece, never off
/// the event stream racing beside it.
struct TaskFailure {
    error: behaviour::Error,
    node: Option<Node>,
}

/// One consumer's or tap's stream, run to its end. Every error it can end
/// on — its own or a panic caught here — is told naming the attachment it
/// failed on.
async fn consume_stream(
    uuid: Uuid,
    label: String,
    port: &'static str,
    with: Consumer,
    receiver: Receiver<Value>,
) -> Result<(), TaskFailure> {
    let outcome = catch_panic(with(receiver)).await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(payload) => Err(panicked(payload)),
    };
    outcome.map_err(|error| TaskFailure {
        error: format!("a consumer of node {label} ({uuid}) `{port}`: {error}").into(),
        node: Some(Node { uuid, label }),
    })
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
/// Every event names its subject — the run, or the node instance and
/// port — and a node's status is derivable from these events alone.
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
    RunFinished { outcome: RunOutcome },
}

/// The run's outcome, as run finished tells it.
#[derive(Debug)]
pub enum RunOutcome {
    /// Every node completed; every consumed stream ended.
    Complete,
    /// The first error ended the run — the same report [`Run::start`]
    /// returns — with the node instance the failure belongs to, named
    /// where the failing task knew it, so what happened and where travel
    /// in one piece. The one failure naming no node is a run task lost
    /// without reporting, where no node is known.
    Failed { error: String, node: Option<Node> },
    /// A stop asked on the attached channel ended the run: the remaining
    /// work stopped where it stands, whatever was in flight abandoned.
    Stopped,
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
                RunOutcome::Failed { error, .. } => format!("run failed: {error}"),
                RunOutcome::Stopped => "run stopped".to_owned(),
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
    if let Some(text) = value.get::<String>() {
        return format!("{text:?}");
    }
    if let Some(text) = crate::scalars::scalar_text(value) {
        return text;
    }
    match crate::registry::data_type_by_id(value.type_id()) {
        Some(data_type) => format!("a {} value", data_type.name),
        None => format!("a value of id {}", value.type_id()),
    }
}
