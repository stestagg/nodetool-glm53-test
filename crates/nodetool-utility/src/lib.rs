//! The first-party library of trivial utility nodes: an `If` router, a
//! `Format`-to-string node, and a `Select` chooser. The crate is a plugin
//! like any other: its only dependency is `nodetool`, its node types
//! are declared through the one `node_type!` registration path with their
//! behaviour through the one authoring API, and linking it into a binary is
//! the whole integration step. Core branches on nothing about which nodes
//! these are; they are ordinary node types through and through.
//!
//! The router's and the formatter's value ports declare the union of the
//! base scalar types, so a value of any base scalar routes and formats; a
//! type outside the union reaches them only through a declared conversion,
//! or fails to compile. Nothing converts to `String` automatically — Format
//! is the visible step. Select chooses between `String`s alone: the one
//! type anything needs it for, and widening it is a port family's job, not
//! a second declaration's.

use std::collections::VecDeque;

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::carried_check;
use nodetool::compile::CompiledNode;
use nodetool::node_type;
use nodetool::registry;
use nodetool::scalars;
use nodetool::Value;

/// The plain string form of a base scalar value: the payload as its Rust
/// `Display` renders it, a string as itself. `None` for a payload that is not
/// a base scalar — a value a compiled connection could not have delivered.
fn string_form(value: &Value) -> Option<String> {
    macro_rules! scalars {
        ($($ty:ty),* $(,)?) => {$(
            if let Some(form) = value.get::<$ty>().map(ToString::to_string) {
                return Some(form);
            }
        )*};
    }
    scalars!(String, bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
    None
}

/// Routes each value to exactly one of its two outputs as the condition
/// selects. Every arrival fires a run — a value arrival routes the arriving
/// value, a condition arrival re-routes the held value under the new
/// condition — the one pairing semantics every node rides; the branch the
/// decision does not select receives nothing, and neither output waits on the
/// other.
struct If;

#[async_trait]
impl Behaviour for If {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let condition = io
            .input("condition")
            .current()
            .and_then(|value| value.get::<bool>().copied())
            .expect("the run is gated on this input's first value");
        let value = io
            .input("value")
            .current()
            .expect("the run is gated on this input's first value")
            .clone();
        io.output(if condition { "then" } else { "else" })
            .emit(value)
            .await;
        Ok(Flow::Continue)
    }
}

fn if_behaviour(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(If)
}

/// Converts each incoming value to a `String`: with a format template, the
/// value's string form substituted at the template's first `{}` — a template
/// without one emits itself verbatim, the value going nowhere; an empty
/// template leaves the value's plain string form. The template's own arrival
/// fills its held value like any other. The template input must deliver a
/// first value before any run fires — in a graph file, the `template`
/// parameter, `""` for the plain form; left unconnected and unset, the node
/// never fires and the run hangs.
struct Format;

#[async_trait]
impl Behaviour for Format {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let value = io
            .input("value")
            .current()
            .expect("the run is gated on this input's first value")
            .clone();
        let form = string_form(&value).ok_or_else(|| {
            let name = registry::data_type_by_id(value.type_id())
                .map(|data_type| data_type.name)
                .unwrap_or("an unregistered type");
            format!("the value of type {name} is not a base scalar this node formats")
        })?;
        let text = match io
            .input("template")
            .current()
            .and_then(|v| v.get::<String>())
        {
            Some(template) if !template.is_empty() => template.replacen("{}", &form, 1),
            _ => form,
        };
        io.output("text")
            .emit(Value::new(scalars::STRING, text))
            .await;
        Ok(Flow::Continue)
    }
}

fn format_behaviour(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Format)
}

/// How Select reads one of its inputs. A connection's arrivals are
/// buffered, and a pairing consumes one from the front of each: reading
/// the current value instead would pair a choice against whichever
/// neighbour happened to arrive last. A parameter's literal is held — it
/// is always there, so it is read where it lies and never consumed, and it
/// never gates a pairing.
enum Carriage {
    Buffered(VecDeque<Value>),
    Held,
}

impl Carriage {
    /// The carriage the named input rides on this instance: held where a
    /// parameter literal carries it, buffered where the wiring feeds it.
    /// An input neither carries is a compile error, so nothing reaches a
    /// run on an empty buffer that will stay empty.
    fn of(name: &'static str, compiled: &CompiledNode) -> Carriage {
        if compiled.parameters.contains_key(name) {
            Carriage::Held
        } else {
            Carriage::Buffered(VecDeque::new())
        }
    }

    /// Take in this input's arrival. A held input needs nothing: the
    /// arrival is already the current value the pairing reads.
    fn receive(&mut self, io: &mut Io<'_>, name: &str) {
        if let Carriage::Buffered(queue) = self {
            let arrival = io
                .input(name)
                .current()
                .expect("the run is gated on this input's first value")
                .clone();
            queue.push_back(arrival);
        }
    }

    /// The value the next pairing would take, `None` while a buffer waits
    /// on its next arrival.
    fn peek(&self, io: &mut Io<'_>, name: &str) -> Option<Value> {
        match self {
            Carriage::Buffered(queue) => queue.front().cloned(),
            Carriage::Held => io.input(name).current().cloned(),
        }
    }

    /// Drop what the pairing just emitted from: a buffer pops in lockstep
    /// with its siblings, a held value stays.
    fn consume(&mut self) {
        if let Carriage::Buffered(queue) = self {
            queue.pop_front();
        }
    }
}

/// The aligned chooser: per triple of a `choose` flag and one value from
/// each candidate, the matching candidate's value — `then` when the flag
/// holds, `else` when it does not. Its inputs each carry one value per
/// choice, in choice order, so the k-th of each belongs to the same
/// choice; buffering the connections and pairing their fronts is what
/// keeps that true whatever order the streams interleave in. A candidate a
/// literal carries simply sits under every choice.
struct Select {
    choose: Carriage,
    then: Carriage,
    else_: Carriage,
}

#[async_trait]
impl Behaviour for Select {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        if let Trigger::Arrival(name) = trigger {
            match name {
                "choose" => self.choose.receive(io, name),
                "then" => self.then.receive(io, name),
                "else" => self.else_.receive(io, name),
                other => panic!("this node declares no input port `{other}`"),
            }
        }
        // One arrival buffers one value, so a run can complete at most one
        // pairing; the arrivals behind it fire their own runs in turn. A
        // loop here would never end on an instance whose every input is
        // held — nothing to consume, the pairing always ready.
        if let (Some(choice), Some(then), Some(otherwise)) = (
            self.choose.peek(io, "choose"),
            self.then.peek(io, "then"),
            self.else_.peek(io, "else"),
        ) {
            let choice = choice
                .get::<bool>()
                .copied()
                .expect("the compiler type-checked the flag against the declared bool port");
            self.choose.consume();
            self.then.consume();
            self.else_.consume();
            io.output("value")
                .emit(if choice { then } else { otherwise })
                .await;
        }
        Ok(Flow::Continue)
    }
}

fn select_behaviour(compiled: &CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Select {
        choose: Carriage::of("choose", compiled),
        then: Carriage::of("then", compiled),
        else_: Carriage::of("else", compiled),
    })
}

fn select_check(compiled: &CompiledNode) -> Vec<String> {
    carried_check(compiled, &["choose", "then", "else"])
}

node_type! {
    type_ref: "utility/select",
    label: "Select",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M2 3h4l5 5h3m0 0-2-2m2 2-2 2M2 13h4l3-3" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
    plugin: "utility",
    behaviour: select_behaviour,
    check_parameters: select_check,
    inputs: [ choose: "bool", then: "String", else: "String" ],
    outputs: [ value: "String" ],
}

node_type! {
    type_ref: "utility/if",
    label: "If",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M2 8h4m0 0c3 0 3-5 8-5M6 8c3 0 3 5 8 5" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
    plugin: "utility",
    behaviour: if_behaviour,
    inputs: [ condition: "bool", value: ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "String"] ],
    outputs: [
        then: ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "String"],
        else: ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "String"],
    ],
}

node_type! {
    type_ref: "utility/format",
    label: "Format",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><text x="2" y="12" font-size="10" fill="#333">f{}</text></svg>"##,
    plugin: "utility",
    behaviour: format_behaviour,
    inputs: [
        value: ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "String"],
        template: "String",
    ],
    outputs: [ text: "String" ],
}
