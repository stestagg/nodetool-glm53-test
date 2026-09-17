//! The first-party library of trivial utility nodes: an `If` router and a
//! `Format`-to-string node — the only two the requirements name. The crate is
//! a plugin like any other: its only dependency is `nodetool`, its node types
//! are declared through the one `node_type!` registration path with their
//! behaviour through the one authoring API, and linking it into a binary is
//! the whole integration step. Core branches on nothing about which nodes
//! these are; they are ordinary node types through and through.
//!
//! Both nodes' value ports declare the union of the base scalar types, so a
//! value of any base scalar routes and formats; a type outside the union
//! reaches them only through a declared conversion, or fails to compile.
//! Nothing converts to `String` automatically — Format is the visible step.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::node_type;
use nodetool::scalars;
use nodetool::Value;

/// The plain string form of a base scalar value: the payload as its Rust
/// `Display` renders it, a string as itself. `None` for a payload that is not
/// a base scalar — a value a compiled connection could not have delivered.
fn string_form(value: &Value) -> Option<String> {
    if let Some(text) = value.get::<String>() {
        return Some(text.clone());
    }
    macro_rules! scalars {
        ($($ty:ty),* $(,)?) => {$(
            if let Some(form) = value.get::<$ty>().map(ToString::to_string) {
                return Some(form);
            }
        )*};
    }
    scalars!(bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
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

fn if_behaviour() -> Box<dyn Behaviour> {
    Box::new(If)
}

/// Converts each incoming value to a `String`: with a format template, the
/// value's string form substituted at the template's first `{}`; an empty
/// template — the parameter left unset — leaves the value's plain string
/// form. The template's own arrival fills its held value like any other.
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
            format!(
                "the value of type id {} is not a base scalar this node formats",
                value.type_id()
            )
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

fn format_behaviour() -> Box<dyn Behaviour> {
    Box::new(Format)
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
