//! The fizzbuzz node library: a counter source, the six condition
//! comparisons beside the divisibility condition, the case selection that
//! turns the count and its divisibility streams into the fizzbuzz string,
//! and the output terminus — the nodes the fizzbuzz graph runs on.
//!
//! The library is the first written against the generic-port idiom: one
//! node type per node, its numeric ports declared as one port family —
//! the base numeric set — that the compiler resolves to a single concrete
//! type from the instance's connections and parameter literals. The
//! numeric logic is written once, generic over `Numeric`, and stamped per
//! resolved type; the behaviour reads values as that concrete Rust type,
//! with no dispatch on values. Core branches on nothing about which nodes
//! these are: they are ordinary node types through and through,
//! registered through the one `node_type!` path.
//!
//! Nothing here prints a run's product: printing is the binary's terminal
//! rule. The collected vec is the output node's own state, for a program
//! that wants the run's product collected.

use std::collections::VecDeque;
use std::marker::PhantomData;

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::compile::CompiledNode;
use nodetool::node_type;
use nodetool::scalars;
use nodetool::Value;

use crate::numeric::{
    for_numeric, numeric_input, numeric_value, sequence_continues, Numeric, NUMERICS,
    NUMERIC_FAMILY,
};

/// The generic-port idiom, plugin-side: the numeric family the fizzbuzz
/// nodes are written against, and the `Numeric` trait the generic bodies
/// are written over.
pub mod numeric;

/// The counter's sequence logic, generic over the family member the
/// compiler resolved: start, then start+step, and so on while the
/// accumulation has not passed stop in step's direction. The whole
/// sequence is one run; when it returns, the node completes, and a
/// later-arriving start, stop, or step value is simply of no effect — the
/// node is complete, and nothing restarts it.
struct CounterLogic<T: Numeric> {
    _member: PhantomData<T>,
}

#[async_trait]
impl<T: Numeric> Behaviour for CounterLogic<T> {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let start = numeric_input::<T>(io, "start");
        let stop = numeric_input::<T>(io, "stop");
        let step = numeric_input::<T>(io, "step");
        if step == T::zero() {
            return Err("input `step` streamed a zero step — the sequence would never move".into());
        }
        let mut value = start;
        while sequence_continues(value, stop, step) {
            io.output("count").emit(numeric_value(value)).await;
            value = match value.add_checked(step) {
                Some(next) => next,
                None => break,
            };
        }
        Ok(Flow::Complete)
    }
}

fn counter_logic<T: Numeric>(_compiled: &CompiledNode) -> Box<dyn Behaviour> {
    Box::new(CounterLogic {
        _member: PhantomData::<T>,
    })
}

fn counter(compiled: &CompiledNode) -> Box<dyn Behaviour> {
    for_numeric!(compiled, counter_logic)
}

/// The compile-time form of the streamed zero step: a `step` parameter of
/// zero is rejected before the run, the way the behaviour would reject it
/// mid-stream.
fn counter_check(compiled: &CompiledNode) -> Vec<String> {
    if compiled.families.contains_key(NUMERIC_FAMILY) {
        let mut errors = carried_check(compiled, &["start", "stop", "step"]);
        errors.extend(for_numeric!(compiled, counter_step_check));
        errors
    } else {
        // The family itself failed to compile; it reported its own error.
        Vec::new()
    }
}

/// The compile-time form of an input the behaviour reads but nothing
/// carries: neither a connection feeds it nor a parameter holds it, so the
/// node's gate waits on an input whose stream is empty — the run would
/// stall there without end, with nothing to read and no error to show.
fn carried_check(compiled: &CompiledNode, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !compiled.parameters.contains_key(*name) && !compiled.fed.contains(*name))
        .map(|name| {
            format!("input `{name}` holds no parameter value and no connection feeds it — the node would never fire")
        })
        .collect()
}

fn counter_step_check<T: Numeric>(compiled: &CompiledNode) -> Vec<String> {
    compiled
        .parameters
        .get("step")
        .filter(|step| step.value.get::<T>() == Some(&T::zero()))
        .map(|_| "input `step` holds a zero parameter — the sequence would never move".to_owned())
        .into_iter()
        .collect()
}

/// The condition's comparison logic, generic over the family member the
/// compiler resolved: each arrival is paired against the other input's
/// held value, the comparison emits its boolean, and the node completes by
/// the default rule. One shared behaviour; the six condition node types
/// carry the six operations.
struct ConditionLogic<T: Numeric> {
    _member: PhantomData<T>,
    op: fn(T, T) -> bool,
}

#[async_trait]
impl<T: Numeric> Behaviour for ConditionLogic<T> {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let a = numeric_input::<T>(io, "a");
        let b = numeric_input::<T>(io, "b");
        let result = (self.op)(a, b);
        io.output("result")
            .emit(Value::new(scalars::BOOL, result))
            .await;
        Ok(Flow::Continue)
    }
}

/// One condition per operation, each stamped from the one shared
/// behaviour: the macro carries the operation in the closure it stamps
/// into the logic, and declares the node type under the condition
/// sub-group. A per-instance operation *setting* would be a new descriptor
/// concept with one consumer; the six declarations carry it instead.
macro_rules! conditions {
    ($($ctor:ident, $behaviour:ident : $type_ref:literal, $label:literal, $compare:expr;)*) => {$(
        fn $ctor<T: Numeric>(_compiled: &CompiledNode) -> Box<dyn Behaviour> {
            Box::new(ConditionLogic { _member: PhantomData::<T>, op: $compare })
        }
        fn $behaviour(compiled: &CompiledNode) -> Box<dyn Behaviour> {
            for_numeric!(compiled, $ctor)
        }
        node_type! {
            type_ref: $type_ref,
            label: $label,
            icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M2 8h12M4 4l-3 4 3 4M12 4l3 4-3 4" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
            plugin: "fizzbuzz",
            sub_group: "condition",
            behaviour: $behaviour,
            inputs: [ a: ["numeric", NUMERICS], b: ["numeric", NUMERICS] ],
            outputs: [ result: "bool" ],
        }
    )*};
}

conditions! {
    eq_of, condition_eq: "fizzbuzz/eq", "Equal", |a, b| a == b;
    ne_of, condition_ne: "fizzbuzz/ne", "Not equal", |a, b| a != b;
    lt_of, condition_lt: "fizzbuzz/lt", "Less", |a, b| a < b;
    le_of, condition_le: "fizzbuzz/le", "Less or equal", |a, b| a <= b;
    gt_of, condition_gt: "fizzbuzz/gt", "Greater", |a, b| a > b;
    ge_of, condition_ge: "fizzbuzz/ge", "Greater or equal", |a, b| a >= b;
}

/// The divisibility comparison: one boolean per `a` arrival, the count's,
/// paired against the held `b`, the divisor. A `b` arrival only resets the
/// held divisor. The general condition's pairing would re-emit on that
/// arrival too — a duplicate no downstream pairing could tell from a
/// count's own boolean — while this node's consumers pair its stream with
/// the `a` stream one value per count. One boolean per count, then,
/// whatever form the divisor takes.
struct DivisibleLogic<T: Numeric> {
    _member: PhantomData<T>,
}

#[async_trait]
impl<T: Numeric> Behaviour for DivisibleLogic<T> {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        if let Trigger::Arrival("a") = trigger {
            let a = numeric_input::<T>(io, "a");
            let b = numeric_input::<T>(io, "b");
            let divisible = b != T::zero() && a % b == T::zero();
            io.output("result")
                .emit(Value::new(scalars::BOOL, divisible))
                .await;
        }
        Ok(Flow::Continue)
    }
}

fn divisible_of<T: Numeric>(_compiled: &CompiledNode) -> Box<dyn Behaviour> {
    Box::new(DivisibleLogic {
        _member: PhantomData::<T>,
    })
}

fn divisible_behaviour(compiled: &CompiledNode) -> Box<dyn Behaviour> {
    for_numeric!(compiled, divisible_of)
}

fn divisible_check(compiled: &CompiledNode) -> Vec<String> {
    carried_check(compiled, &["a", "b"])
}

node_type! {
    type_ref: "fizzbuzz/divisible",
    label: "Divisible",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M12 4 4 12" stroke="#333" stroke-width="1.5" fill="none"/><circle cx="5" cy="5" r="2" stroke="#333" stroke-width="1.5" fill="none"/><circle cx="11" cy="11" r="2" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
    plugin: "fizzbuzz",
    sub_group: "condition",
    behaviour: divisible_behaviour,
    check_parameters: divisible_check,
    inputs: [ a: ["numeric", NUMERICS], b: ["numeric", NUMERICS] ],
    outputs: [ result: "bool" ],
}

/// The case selection's alignment. Its three inputs each arrive one value
/// per count, in count order — the k-th arrival of each belongs to the
/// same count — so the node buffers each stream's arrivals and, the moment
/// all three hold a value, pairs their fronts and emits that count's
/// fizzbuzz string. The pairing is the node's own, not the driver's
/// held-value rule: reading the other inputs' current values would pair a
/// count against whichever neighbours happened to arrive last.
struct CaseSelection<T: Numeric> {
    _member: PhantomData<T>,
    counts: VecDeque<T>,
    fizz: VecDeque<bool>,
    buzz: VecDeque<bool>,
}

#[async_trait]
impl<T: Numeric> Behaviour for CaseSelection<T> {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        match trigger {
            Trigger::Arrival("count") => self.counts.push_back(numeric_input::<T>(io, "count")),
            Trigger::Arrival("fizz") => self.fizz.push_back(flag_input(io, "fizz")),
            Trigger::Arrival("buzz") => self.buzz.push_back(flag_input(io, "buzz")),
            Trigger::Arrival(_) | Trigger::Start => {}
        }
        while self.counts.front().is_some()
            && self.fizz.front().is_some()
            && self.buzz.front().is_some()
        {
            let count = self.counts.pop_front().expect("the front holds a value");
            let fizz = self.fizz.pop_front().expect("the front holds a value");
            let buzz = self.buzz.pop_front().expect("the front holds a value");
            io.output("text")
                .emit(Value::new(scalars::STRING, case_text(count, fizz, buzz)))
                .await;
        }
        Ok(Flow::Continue)
    }
}

fn case_selection<T: Numeric>(_compiled: &CompiledNode) -> Box<dyn Behaviour> {
    Box::new(CaseSelection {
        _member: PhantomData::<T>,
        counts: VecDeque::new(),
        fizz: VecDeque::new(),
        buzz: VecDeque::new(),
    })
}

fn case_behaviour(compiled: &CompiledNode) -> Box<dyn Behaviour> {
    for_numeric!(compiled, case_selection)
}

fn case_check(compiled: &CompiledNode) -> Vec<String> {
    carried_check(compiled, &["count", "fizz", "buzz"])
}

fn case_text<T: Numeric>(count: T, fizz: bool, buzz: bool) -> String {
    match (fizz, buzz) {
        (true, true) => "FizzBuzz".to_owned(),
        (true, false) => "Fizz".to_owned(),
        (false, true) => "Buzz".to_owned(),
        (false, false) => count.to_string(),
    }
}

/// The input's current value as its declared `bool` — the run is gated on
/// the input's first value, and the compiler type-checks the port against
/// its declaration.
fn flag_input(io: &mut Io<'_>, name: &str) -> bool {
    io.input(name)
        .current()
        .and_then(|value| value.get::<bool>().copied())
        .expect("the run is gated on this input's first value")
}

node_type! {
    type_ref: "fizzbuzz/case",
    label: "Case selection",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M2 4h4l6 4M2 8h10M2 12h4l6-4" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
    plugin: "fizzbuzz",
    behaviour: case_behaviour,
    check_parameters: case_check,
    inputs: [ count: ["numeric", NUMERICS], fizz: "bool", buzz: "bool" ],
    outputs: [ text: "String" ],
}

/// The output's collection: every string that arrives, in arrival order,
/// completing when its input completes. The vec is the node's own state;
/// how a consumer reaches it at run end is the run's business, not this
/// node's.
struct Output {
    collected: Vec<String>,
}

#[async_trait]
impl Behaviour for Output {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let text = io
            .input("text")
            .current()
            .and_then(|value| value.get::<String>().cloned())
            .expect("the run is gated on this input's first value");
        self.collected.push(text);
        Ok(Flow::Continue)
    }
}

fn output(_compiled: &CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Output {
        collected: Vec::new(),
    })
}

node_type! {
    type_ref: "fizzbuzz/counter",
    label: "Counter",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M3 13V7l4 3V7l4 3V7l2 1.5" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
    plugin: "fizzbuzz",
    behaviour: counter,
    check_parameters: counter_check,
    inputs: [ start: ["numeric", NUMERICS], stop: ["numeric", NUMERICS], step: ["numeric", NUMERICS] ],
    outputs: [ count: ["numeric", NUMERICS] ],
}

node_type! {
    type_ref: "fizzbuzz/output",
    label: "Output",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M2 8h9m0 0-3-3m3 3-3 3M13 4v8" stroke="#333" stroke-width="1.5" fill="none"/></svg>"##,
    plugin: "fizzbuzz",
    behaviour: output,
    inputs: [ text: "String" ],
    outputs: [],
}

#[cfg(test)]
mod tests;
