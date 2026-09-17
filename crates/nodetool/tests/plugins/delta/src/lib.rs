//! Node type fixtures for the engine tests: a self-driving source, a
//! pairer, an echo, and a failer — chosen so whole-graph wiring, a literal
//! held across a graph's arrivals, streaming consumption, and fail-fast are
//! each reachable from a small graph. The tests observe runs through the
//! run's own consumer attachments, so the behaviours only transform and
//! emit.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::node_type;
use nodetool::scalars;
use nodetool::Value;

struct Counter;

#[async_trait]
impl Behaviour for Counter {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // More than two hand-offs can hold, so a stalled consumer stalls
        // its whole upstream chain mid-stream: the tests can hold nodes
        // uncompleted at a known point.
        for value in 1..=50 {
            io.output("out").emit(Value::new(scalars::I32, value)).await;
        }
        Ok(Flow::Complete)
    }
}

fn counter() -> Box<dyn Behaviour> {
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

fn pairer() -> Box<dyn Behaviour> {
    Box::new(Pairer)
}

struct Echo;

#[async_trait]
impl Behaviour for Echo {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let value = current_i32(io, "value");
        io.output("value")
            .emit(Value::new(scalars::I32, value))
            .await;
        Ok(Flow::Continue)
    }
}

fn echo() -> Box<dyn Behaviour> {
    Box::new(Echo)
}

struct Failer;

#[async_trait]
impl Behaviour for Failer {
    async fn process(&mut self, _trigger: Trigger, _io: &mut Io<'_>) -> Result<Flow, Error> {
        Err("the failer ran".into())
    }
}

fn failer() -> Box<dyn Behaviour> {
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
    type_ref: "delta/echo",
    label: "Echo",
    icon: "<svg/>",
    plugin: "delta",
    behaviour: echo,
    inputs: [ value: "i32" ],
    outputs: [ value: "i32" ],
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
