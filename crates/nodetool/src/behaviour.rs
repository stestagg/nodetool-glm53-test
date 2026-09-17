//! The node authoring API, and the stream semantics every behaviour
//! programs against.
//!
//! A node type's whole being is declared through the one registration path:
//! its descriptor ([`crate::node_type!`]) names the ports, and the descriptor's
//! [`BehaviourFn`] builds the behaviour each instance runs. There is no
//! second behaviour mechanism, and the driver treats every node identically:
//! it branches on nothing but the shape of the streams themselves.
//!
//! The semantics, settled once so that every consumer inherits them:
//!
//! * **Arrivals and the current value.** Each input presents both faces.
//!   [`Input::next`] awaits the next *arrival*: each new value exactly once,
//!   in order, then the stream's end. [`Input::current`] reads the *current
//!   value*: the latest value seen, none before the first arrival, still
//!   readable after the input completes. Awaiting arrivals and reading the
//!   current value are the two primitives everything else composes from.
//! * **One arrival, one run.** An arrival on any input fires the behaviour
//!   once. At the moment a run fires, the arrival has become its input's
//!   current value, so reading every input's current value pairs the arrival
//!   with the held value of every other input as of that moment. Runs are
//!   gated until each input has delivered a first value — before that,
//!   arrivals queue instead of firing. Arrivals landing while a run is still
//!   processing fire as further runs, serially and in arrival order: a
//!   behaviour is never re-entered against itself, so it writes its own
//!   state without synchronisation. A behaviour needing different arrival
//!   semantics consumes arrivals directly — [`Input::next`] works from
//!   inside a run — through the same faces, never a parallel path; an
//!   arrival a behaviour consumes directly never reaches the driver, so it
//!   fires no run.
//! * **Sources.** A node with no inputs has no arrivals to wait on: it
//!   drives itself, fired once ([`Trigger::Start`]) when the node begins,
//!   and completes when that run returns. A constant is nothing more than
//!   such a stream that yields once and completes.
//! * **Completion.** A stream ends with no more data ever arriving after
//!   it. A node completes, by default, when every input has ended and every
//!   arrived value has been processed; its logic may complete earlier by
//!   returning [`Flow::Complete`]. On completion the node's outputs end, so
//!   completion propagates downstream. An input nothing feeds — unconnected
//!   — is the degenerate stream: it completes at once and never delivers a
//!   value, so a node holding one never fires. An input fixed by a
//!   parameter is instead fed a stream that yields once and completes: its
//!   arrival fires the node like any arrival, and the held value pairs
//!   against every later arrival. If every input is degenerate the node
//!   completes immediately, and if any other input is connected the node
//!   neither fires nor completes: the run hangs, the fed input's hand-off
//!   filling until its upstream stalls.
//! * **Backpressure.** A value crosses from an emitting node to each
//!   connected downstream input through a bounded hand-off
//!   ([`HANDOFF_CAPACITY`], one fixed policy, no author-facing knobs):
//!   emitting waits while a hand-off is full, so a fast producer cannot
//!   outrun a slow consumer into unbounded memory. Fan-out — one emission to
//!   every connected downstream input — is the driver's routing, not the
//!   behaviour's concern.
//! * **Errors.** An error a behaviour surfaces is raised on the run path,
//!   never swallowed into a silent skip or a dropped value.
//!
//! The behaviour itself is one method, [`Behaviour::process`], called once
//! per run with the node's live ports ([`Io`]) — inputs read by name
//! (arrivals or current value), outputs emitted to. Behaviour code reads
//! values as the concrete Rust types its ports declared: every value on a
//! stream is a [`Value`], which names the data type it is an instance of.

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::future::{pending, poll_fn};
use std::task::Poll;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{ConvertFn, Value};

/// The fixed capacity of the bounded hand-off between an emitting node and
/// each downstream input: one policy for every connection, so memory stays
/// bounded when a consumer is slower than its producer.
pub const HANDOFF_CAPACITY: usize = 16;

pub use tokio::sync::mpsc::{Receiver, Sender};

/// An error a behaviour surfaces. It propagates on the run path; what a run
/// then does with it is the engine's decision.
pub type Error = Box<dyn StdError + Send + Sync>;

/// The behaviour of one node type's instances: what the driver fires once
/// per arrival, serially, never re-entered against itself.
///
/// Implementations hold whatever per-instance state the node needs. Reads
/// and writes go through `io`; state on `self` needs no synchronisation —
/// no two runs of one node overlap.
#[async_trait]
pub trait Behaviour: Send {
    /// One run of the node's processing, fired by `trigger`. Read every
    /// input's current value to pair the arrival with the held values of
    /// the other inputs as of this moment; emit on the outputs as the
    /// results form. Returning [`Flow::Complete`] ends the node now;
    /// [`Flow::Continue`] leaves it to the next trigger.
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error>;
}

/// What fired the behaviour run now starting.
pub enum Trigger {
    /// The node's own start: a node with no inputs is fired once,
    /// untriggered — there is no stream to wait on. Such a node completes
    /// when its run returns, whatever the flow asks: nothing can ever
    /// trigger it again.
    Start,
    /// An arrival on the named input; the arrival is that input's current
    /// value as the run begins.
    Arrival(&'static str),
}

/// What one behaviour run asks of the driver.
pub enum Flow {
    /// Carry on: the next arrival fires the next run.
    Continue,
    /// The node's logic is done: end the node now, its outputs with it —
    /// arrivals still queued or in flight are left unprocessed.
    Complete,
}

/// Open one bounded hand-off: the sender an emitting node's output holds,
/// the receiver the downstream input holds.
pub fn handoff() -> (Sender<Value>, Receiver<Value>) {
    mpsc::channel(HANDOFF_CAPACITY)
}

/// One live input of a running node: the two faces every input presents.
pub struct Input {
    name: &'static str,
    receiver: mpsc::Receiver<Value>,
    current: Option<Value>,
}

impl Input {
    /// An input fed by the stream one upstream node emits into `receiver`.
    pub fn new(name: &'static str, receiver: mpsc::Receiver<Value>) -> Input {
        Input {
            name,
            receiver,
            current: None,
        }
    }

    /// An input nothing feeds — the degenerate stream: it completes at once
    /// and never delivers a value, so a node holding it never fires.
    pub fn unconnected(name: &'static str) -> Input {
        let (_, receiver) = handoff();
        Input::new(name, receiver)
    }

    /// Await the next arrival: each new value exactly once, in order, then
    /// the stream's end. Each arrival becomes the input's current value as
    /// it is delivered.
    ///
    /// The driver draws from this same stream: it takes each input's first
    /// arrival to open the gate, firing it as a run like any other — so
    /// direct consumption starts at that input's second value. And a run
    /// queued on an older arrival resets the current value to that arrival
    /// as it fires, so between direct reads the current value can move
    /// backwards. A behaviour mixing the two faces owns both effects.
    pub async fn next(&mut self) -> Option<Value> {
        let value = self.receiver.recv().await?;
        self.current = Some(value.clone());
        Some(value)
    }

    /// The current value: the latest value seen, none before the first
    /// arrival, still readable after the input completes.
    pub fn current(&self) -> Option<&Value> {
        self.current.as_ref()
    }

    /// Deliver an arrival into the held slot — the driver, as the value is
    /// delivered to the node and again when its run fires.
    fn arrive(&mut self, value: Value) {
        self.current = Some(value);
    }
}

/// One live output of a running node: an emission is delivered to every
/// connected downstream input. A downstream whose connection rides a
/// declared conversion receives the converted value — the conversion is
/// wiring, applied as the value crosses, never the behaviour's concern.
pub struct Output {
    name: &'static str,
    senders: Vec<(mpsc::Sender<Value>, Option<ConvertFn>)>,
    watch: Option<EmissionWatch>,
}

/// Handed each emission beside the deliveries: the port's name and the
/// value as the behaviour emitted it — before any conversion a connection
/// rides, whatever the deliveries then do with it. Handing is a one-way
/// tell that never blocks the emission; what it carries the flow to is
/// the installer's business, not the output's.
pub type EmissionWatch = Box<dyn Fn(&'static str, &Value) + Send + Sync>;

impl Output {
    /// An output with no downstream connections yet.
    pub fn new(name: &'static str) -> Output {
        Output {
            name,
            senders: Vec::new(),
            watch: None,
        }
    }

    /// Install the emission watcher beside this output's deliveries —
    /// wiring an engine installs, never a behaviour's concern. There is
    /// one watcher per output: installing a second is a bug — it would
    /// silently replace the installed wiring — so it panics.
    pub fn watch_emissions(&mut self, watch: EmissionWatch) {
        assert!(
            self.watch.is_none(),
            "this output already has an emission watcher; replacing installed wiring is a bug"
        );
        self.watch = Some(watch);
    }

    /// Wire one downstream input to this output: from here on, every
    /// emission reaches it, converted first when the connection rides a
    /// declared conversion.
    pub fn connect(&mut self, sender: mpsc::Sender<Value>, convert: Option<ConvertFn>) {
        self.senders.push((sender, convert));
    }

    /// Emit a value: delivered to every connected downstream input, waiting
    /// on each while its hand-off is full. The emission watcher, if one is
    /// installed, is told of the value as emitted and never holds the
    /// emission up. A downstream that has ended — its input gone —
    /// receives nothing more: the delivery is skipped, which is that
    /// stream ending, not an error. A conversion that refuses a value
    /// (`None`) ends the run, reported with the node and port.
    pub async fn emit(&mut self, value: Value) {
        if let Some(watch) = &self.watch {
            watch(self.name, &value);
        }
        for (sender, convert) in &mut self.senders {
            let value = match convert {
                Some(convert) => convert(&value).unwrap_or_else(|| {
                    panic!(
                        "the conversion declared on output `{}` does not take this value",
                        self.name
                    )
                }),
                None => value.clone(),
            };
            let _ = sender.send(value).await;
        }
    }

    /// End the output: the node completed, so its downstream streams end.
    fn end(&mut self) {
        self.senders.clear();
    }
}

/// The node's live ports, as one behaviour run sees them.
///
/// A port name that this node does not declare is an authoring bug; the
/// lookup panics naming the port, like an index out of bounds.
pub struct Io<'a> {
    inputs: &'a mut [Input],
    outputs: &'a mut [Output],
}

impl<'a> Io<'a> {
    pub fn input(&mut self, name: &str) -> &mut Input {
        match self.inputs.iter_mut().find(|input| input.name == name) {
            Some(input) => input,
            None => panic!("this node declares no input port `{name}`"),
        }
    }

    pub fn output(&mut self, name: &str) -> &mut Output {
        match self.outputs.iter_mut().find(|output| output.name == name) {
            Some(output) => output,
            None => panic!("this node declares no output port `{name}`"),
        }
    }
}

/// Build the behaviour each instance of a node type runs — the authoring
/// API's half of the declaration, together with the descriptor the whole
/// node type.
pub type BehaviourFn = fn() -> Box<dyn Behaviour>;

/// Drive one node: run its behaviour over its inputs and outputs per the
/// stream semantics, until the node completes or its behaviour raises an
/// error.
///
/// The engine instantiates one behaviour per node instance and awaits this
/// once per instance: `Ok(())` is the node completed — its outputs end here,
/// so its downstream streams end — and `Err` is a behaviour error, raised,
/// never swallowed. On an error the outputs are left as they are; what a
/// failed node's downstream then sees is the engine's to decide, not this
/// function's.
pub async fn drive(
    behaviour: &mut dyn Behaviour,
    inputs: &mut [Input],
    outputs: &mut [Output],
) -> Result<(), Error> {
    if inputs.is_empty() {
        // A source: fired once, untriggered. Whatever the flow asks, nothing
        // can ever trigger it again — the node completes when the run ends.
        let mut io = Io { inputs, outputs };
        behaviour.process(Trigger::Start, &mut io).await?;
        return complete(outputs);
    }

    // One trigger per arrival. Until every input has delivered a first
    // value, arrivals queue instead of firing: a run pairs against held
    // values that may not exist yet.
    let mut queued: VecDeque<(usize, Value)> = VecDeque::new();
    let mut delivered = vec![false; inputs.len()];
    // Each input's arrivals queued here, unprocessed. Once one reaches the
    // hand-off's capacity the driver stops polling that input, so its
    // bounded hand-off fills and the upstream stalls there — the driver
    // never pulls a whole stream into its own memory.
    let mut backlog = vec![0usize; inputs.len()];
    loop {
        if delivered.iter().all(|&d| d) {
            if let Some((index, value)) = queued.pop_front() {
                backlog[index] -= 1;
                let name = inputs[index].name;
                inputs[index].arrive(value);
                let mut io = Io { inputs, outputs };
                match behaviour.process(Trigger::Arrival(name), &mut io).await? {
                    Flow::Continue => {}
                    Flow::Complete => return complete(outputs),
                }
                continue;
            }
        }
        match next_arrival(inputs, &backlog).await {
            Some((index, value)) => {
                // The delivery makes it the input's current value: held from
                // here on, so any earlier-queued run already pairs against
                // it once the gate opens.
                inputs[index].arrive(value.clone());
                queued.push_back((index, value));
                delivered[index] = true;
                backlog[index] += 1;
            }
            None => {
                // Every input's stream has ended and drained. With the gate
                // open — or with nothing ever arrived — the node completes:
                // every arrived value is processed. An input that never
                // delivered a first value is the degenerate stream.
                if delivered.iter().all(|&d| d) || !delivered.iter().any(|&d| d) {
                    return complete(outputs);
                }
                // The gate can never open: some input delivered, some never
                // will, and every stream has ended. The node never fires and
                // never completes — the run hangs here, arrived values
                // queued, outputs open.
                return pending().await;
            }
        }
    }
}

/// Every input's stream ended: a completed node's outputs end with it.
fn complete(outputs: &mut [Output]) -> Result<(), Error> {
    for output in outputs {
        output.end();
    }
    Ok(())
}

/// The next arrival across every input, in the order the inputs deliver
/// them — each input's own order preserved. An input whose unprocessed
/// backlog has reached the hand-off's capacity is left alone — polling it
/// would drain the hand-off its upstream needs to stall against — and
/// counts as still waiting, never as ended. `None` once every stream has
/// ended and drained: nothing will ever arrive again.
async fn next_arrival(inputs: &mut [Input], backlog: &[usize]) -> Option<(usize, Value)> {
    poll_fn(|cx| {
        let mut waiting = false;
        for (index, input) in inputs.iter_mut().enumerate() {
            if backlog[index] >= HANDOFF_CAPACITY {
                // Full: leave the value in the hand-off, the upstream
                // stalled on it. Not an end — polling resumes once the
                // backlog drains. An input the gate still waits on can
                // never take this branch: backlog grows only alongside
                // `delivered = true`, so an undelivered input always has
                // backlog 0 and is always polled.
                waiting = true;
                continue;
            }
            match input.receiver.poll_recv(cx) {
                Poll::Ready(Some(value)) => return Poll::Ready(Some((index, value))),
                // Ended: drained and closed. Nothing more from this stream.
                Poll::Ready(None) => continue,
                Poll::Pending => waiting = true,
            }
        }
        if waiting {
            Poll::Pending
        } else {
            Poll::Ready(None)
        }
    })
    .await
}
