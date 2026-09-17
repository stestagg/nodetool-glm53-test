//! Node type fixtures for the engine tests: a self-driving source, a
//! pairer, an f64 echo, a source over a mixed output, and a failer — chosen
//! so whole-graph wiring, a literal held across a graph's arrivals, a
//! conversion riding a connection, and fail-fast are each reachable from a
//! small graph. The tests observe runs through the run's own consumer
//! attachments, so the behaviours only transform and emit.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::node_type;
use nodetool::scalars;
use nodetool::Value;

struct Counter;

#[async_trait]
impl Behaviour for Counter {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // The counter emits more than two hand-offs can hold, so a stalled
        // consumer stalls its whole upstream chain mid-stream: the tests
        // can hold nodes uncompleted at a known point.
        for value in 1..=50 {
            io.output("out").emit(Value::new(scalars::I32, value)).await;
        }
        Ok(Flow::Complete)
    }
}

fn counter(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Counter)
}

/// The input's current value as the i32 its port declared.
fn current_i32(io: &mut Io<'_>, name: &str) -> i32 {
    io.input(name)
        .current()
        .and_then(|value| value.get::<i32>().copied())
        .expect("the run is gated on this input's first value")
}

struct Pairer;

#[async_trait]
impl Behaviour for Pairer {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let sum = current_i32(io, "a") + current_i32(io, "b");
        io.output("sum").emit(Value::new(scalars::I32, sum)).await;
        Ok(Flow::Continue)
    }
}

fn pairer(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Pairer)
}

/// The input's current value as the f64 its port declared.
fn current_f64(io: &mut Io<'_>, name: &str) -> f64 {
    io.input(name)
        .current()
        .and_then(|value| value.get::<f64>().copied())
        .expect("the run is gated on this input's first value")
}

struct EchoF64;

#[async_trait]
impl Behaviour for EchoF64 {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let value = current_f64(io, "value");
        io.output("value")
            .emit(Value::new(scalars::F64, value))
            .await;
        Ok(Flow::Continue)
    }
}

fn echo_f64(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(EchoF64)
}

struct MixedSource;

#[async_trait]
impl Behaviour for MixedSource {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // The String member is declared on the output, so a graph may wire
        // the port through a conversion that takes only the i32 member —
        // and the refusal then surfaces here, mid-run.
        io.output("mixed")
            .emit(Value::new(scalars::STRING, "spoken".to_owned()))
            .await;
        Ok(Flow::Complete)
    }
}

fn mixed_source(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(MixedSource)
}

struct Failer;

#[async_trait]
impl Behaviour for Failer {
    async fn process(&mut self, _trigger: Trigger, _io: &mut Io<'_>) -> Result<Flow, Error> {
        Err("the failer ran".into())
    }
}

fn failer(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Failer)
}

node_type! {
    type_ref: "delta/counter",
    label: "Counter",
    icon: "<svg/>",
    plugin: "delta",
    behaviour: counter,
    inputs: [],
    outputs: [ out: "i32" ],
}

node_type! {
    type_ref: "delta/pairer",
    label: "Pairer",
    icon: "<svg/>",
    plugin: "delta",
    behaviour: pairer,
    inputs: [ a: "i32", b: "i32" ],
    outputs: [ sum: "i32" ],
}

node_type! {
    type_ref: "delta/echo_f64",
    label: "Echo f64",
    icon: "<svg/>",
    plugin: "delta",
    behaviour: echo_f64,
    inputs: [ value: "f64" ],
    outputs: [ value: "f64" ],
}

node_type! {
    type_ref: "delta/mixed_source",
    label: "Mixed source",
    icon: "<svg/>",
    plugin: "delta",
    behaviour: mixed_source,
    inputs: [],
    outputs: [ mixed: ["i32", "String"] ],
}

node_type! {
    type_ref: "delta/failer",
    label: "Failer",
    icon: "<svg/>",
    plugin: "delta",
    behaviour: failer,
    inputs: [ value: "i32" ],
    outputs: [],
}
