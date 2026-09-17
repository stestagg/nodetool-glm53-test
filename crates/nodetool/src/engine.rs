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
//! stream. Or the first error a behaviour or consumer surfaces ends the
//! run, fail-fast: the remaining work stops, and the error names the node
//! instance — its label and uuid — and what went wrong. Mid-run
//! cancellation is engine-internal: in-flight values may still sit in a
//! hand-off when the run ends; the guarantee is that the run ends and the
//! error is the last word, not a frozen instant.
//!
//! A run holds its compiled graph read-only and leaves nothing behind on
//! it: a second run of the same compiled graph, and a run of a recompiled
//! definition, each start clean.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use tokio::task::{JoinError, JoinSet};
use uuid::Uuid;

use crate::behaviour::{self, drive, handoff, Behaviour, Input, Output, Receiver, Sender};
use crate::compile::{CompiledGraph, CompiledNode};
use crate::{ConvertFn, Value};

/// A run of a compiled graph: the engine's one entry point.
///
/// Build it with [`Run::new`], attach consumers with [`Run::consume`] while
/// it is still being built — wiring is fixed at start — and run it with
/// [`Run::start`]. The run borrows its compiled graph, mutates nothing, and
/// leaves nothing behind: run it again and it starts clean.
pub struct Run<'g> {
    graph: &'g CompiledGraph,
    consumers: Vec<ConsumerWiring>,
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
        }
    }

    /// Attach one more downstream to a node output's fan-out — the one
    /// routing mechanism the run already wires, so there is no separate
    /// results path and nothing that waits for the run to end. `with`
    /// receives the stream's receiver; its error ends the run like any
    /// downstream failure, and the run does not end while the consumer is
    /// still consuming. Attaching is for building: past [`Run::start`] the
    /// wiring is fixed.
    ///
    /// A node or port the graph does not declare is an engine bug, the way
    /// a port lookup on a live node is a behaviour bug: it panics naming
    /// the miss.
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
    pub async fn start(self) -> Result<(), behaviour::Error> {
        let Run { graph, consumers } = self;
        let mut set = JoinSet::new();

        // Nothing runs a behaviour-less node: its type declared no behaviour
        // to build. Said before anything is wired, so the run never starts
        // half-wired.
        for (uuid, node) in &graph.nodes {
            if node.node_type.behaviour.is_none() {
                return Err(format!(
                    "node {} ({uuid}) instantiates `{}`, which declares no behaviour to run",
                    name(node),
                    node.node_type.type_ref
                )
                .into());
            }
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
                if let Some(wired) = senders.remove(&(*uuid, port.name)) {
                    for (sender, conversion) in wired {
                        output.connect(sender, conversion);
                    }
                }
                outputs.push(output);
            }
            let behaviour = (node_type.behaviour.expect("checked before wiring"))();
            let label = name(node);
            set.spawn(run_node(*uuid, label, behaviour, inputs, outputs));
        }

        for (consumer, receiver) in consumer_streams {
            let label = name(&graph.nodes[&consumer.node]);
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
        // after aborting the rest.
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    set.abort_all();
                    return Err(error);
                }
                Err(join_error) => {
                    set.abort_all();
                    return Err(lost_run_task(join_error));
                }
            }
        }
        Ok(())
    }
}

/// One node's execution: its behaviour driven over its live ports, per the
/// stream semantics. Its error names the node instance — its label and
/// uuid — and what went wrong.
async fn run_node(
    uuid: Uuid,
    label: String,
    mut behaviour: Box<dyn Behaviour>,
    mut inputs: Vec<Input>,
    mut outputs: Vec<Output>,
) -> Result<(), behaviour::Error> {
    drive(behaviour.as_mut(), &mut inputs, &mut outputs)
        .await
        .map_err(|error| format!("node {label} ({uuid}): {error}").into())
}

/// One parameter literal's stream: yields its value once, then ends. A
/// receiving node gone is that stream ending, not an error.
async fn parameter_stream(value: Value, sender: Sender<Value>) -> Result<(), behaviour::Error> {
    let _ = sender.send(value).await;
    Ok(())
}

/// One consumer's stream, run to its end. Its error names the attachment it
/// failed on.
async fn consume_stream(
    uuid: Uuid,
    label: String,
    port: &'static str,
    with: Consumer,
    receiver: Receiver<Value>,
) -> Result<(), behaviour::Error> {
    with(receiver)
        .await
        .map_err(|error| format!("a consumer of node {label} ({uuid}) `{port}`: {error}").into())
}

/// A run task that neither completed nor reported its error: it died
/// abruptly. A panic is told, never swallowed — the run ends the same
/// fail-fast way.
fn lost_run_task(join_error: JoinError) -> behaviour::Error {
    let reason = match join_error.try_into_panic() {
        Ok(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| {
                    payload
                        .downcast_ref::<&'static str>()
                        .map(|text| (*text).to_owned())
                })
                .unwrap_or_else(|| "unknown panic payload".to_owned());
            format!("a run task panicked: {message}")
        }
        Err(join_error) => {
            format!("a run task was cancelled mid-run: {join_error}")
        }
    };
    reason.into()
}

/// The name a run reports a node by: the instance's label as the definition
/// gave it, or the type's default label when it gave none.
fn name(node: &CompiledNode) -> String {
    node.label
        .clone()
        .unwrap_or_else(|| node.node_type.label.to_owned())
}
