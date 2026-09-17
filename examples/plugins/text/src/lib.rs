//! Example nodetool plugin: flat text nodes, no sub-groups. Every node is
//! declared through the authoring API — descriptor and behaviour together.

use std::time::Duration;

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::scalars;
use nodetool::{node_type, Value};

/// The input's current value as the String its port declared.
fn current_string(io: &mut Io<'_>, name: &str) -> String {
    io.input(name)
        .current()
        .expect("the run is gated on this input's first value")
        .get::<String>()
        .cloned()
        .expect("the input holds its declared String")
}

struct Words;

#[async_trait]
impl Behaviour for Words {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // A self-driving source: fired once at the node's start, its lines
        // emitted, and the node complete when the run returns. It produces
        // on its own schedule — the gap between its lines lets a run's
        // consumers be seen mid-stream, before the second line's parts land.
        io.output("out")
            .emit(Value::new(scalars::STRING, "alpha, beta, gamma".to_owned()))
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        io.output("out")
            .emit(Value::new(scalars::STRING, "one, one, one".to_owned()))
            .await;
        Ok(Flow::Complete)
    }
}

fn words() -> Box<dyn Behaviour> {
    Box::new(Words)
}

struct Drip;

#[async_trait]
impl Behaviour for Drip {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // Like words, but a third line arrives a pause after the second:
        // a run failing on the second line's first word demonstrably
        // withholds it.
        io.output("out")
            .emit(Value::new(scalars::STRING, "alpha, beta, gamma".to_owned()))
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        io.output("out")
            .emit(Value::new(scalars::STRING, "one, one, one".to_owned()))
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        io.output("out")
            .emit(Value::new(
                scalars::STRING,
                "delta, epsilon, zeta".to_owned(),
            ))
            .await;
        Ok(Flow::Complete)
    }
}

fn drip() -> Box<dyn Behaviour> {
    Box::new(Drip)
}

struct Uppercase;

#[async_trait]
impl Behaviour for Uppercase {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let text = current_string(io, "text").to_uppercase();
        io.output("text")
            .emit(Value::new(scalars::STRING, text))
            .await;
        Ok(Flow::Continue)
    }
}

fn uppercase() -> Box<dyn Behaviour> {
    Box::new(Uppercase)
}

struct Split;

#[async_trait]
impl Behaviour for Split {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // Each text arrival splits; the separator's own arrival merely
        // fills its held value — nothing new to split.
        let Trigger::Arrival("text") = trigger else {
            return Ok(Flow::Continue);
        };
        // The run fires after the gate, so each input holds a value; the
        // separator's single value survives its stream's end.
        let text = current_string(io, "text");
        let separator = current_string(io, "separator");
        for part in text.split(separator.as_str()) {
            io.output("parts")
                .emit(Value::new(scalars::STRING, part.to_owned()))
                .await;
        }
        Ok(Flow::Continue)
    }
}

fn split() -> Box<dyn Behaviour> {
    Box::new(Split)
}

/// A guard in the stream: each text arrival passes through unless it is
/// the forbidden value, whose arrival the node rejects with an error.
struct Check;

#[async_trait]
impl Behaviour for Check {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // Each text arrival passes or is rejected; the forbidden literal's
        // own arrival merely fills its held value.
        let Trigger::Arrival("text") = trigger else {
            return Ok(Flow::Continue);
        };
        let text = current_string(io, "text");
        let forbidden = current_string(io, "forbidden");
        if text == forbidden {
            return Err(format!("the value {text:?} is rejected here").into());
        }
        io.output("text")
            .emit(Value::new(scalars::STRING, text))
            .await;
        Ok(Flow::Continue)
    }
}

fn check() -> Box<dyn Behaviour> {
    Box::new(Check)
}

node_type! {
    type_ref: "text/uppercase",
    label: "Uppercase",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><text x="3" y="12" font-size="12" fill="#333">A</text></svg>"##,
    plugin: "text",
    behaviour: uppercase,
    inputs: [ text: "String" ],
    outputs: [ text: "String" ],
}

node_type! {
    type_ref: "text/words",
    label: "Words",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><text x="2" y="12" font-size="10" fill="#333">a b</text></svg>"##,
    plugin: "text",
    behaviour: words,
    inputs: [],
    outputs: [ out: "String" ],
}

node_type! {
    type_ref: "text/drip",
    label: "Drip",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="3" r="1.5" fill="#333"/><circle cx="8" cy="8" r="1.5" fill="#333"/><circle cx="8" cy="13" r="1.5" fill="#333"/></svg>"##,
    plugin: "text",
    behaviour: drip,
    inputs: [],
    outputs: [ out: "String" ],
}

node_type! {
    type_ref: "text/split",
    label: "Split",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M8 2v12M2 8h12" stroke="#333" stroke-width="2"/></svg>"##,
    plugin: "text",
    behaviour: split,
    inputs: [ text: "String", separator: "String" ],
    outputs: [ parts: "String" ],
}

node_type! {
    type_ref: "text/check",
    label: "Check",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M3 8l4 4 6-8" stroke="#333" stroke-width="2" fill="none"/></svg>"##,
    plugin: "text",
    behaviour: check,
    inputs: [ text: "String", forbidden: "String" ],
    outputs: [ text: "String" ],
}
