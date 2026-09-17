//! The fizzbuzz nodes' behaviours, driven one node at a time: the
//! counter's sequence — literal and streamed, inverted range, streamed
//! zero step, completion after exhaustion, no restart on later arrivals —
//! each condition operation across several numeric types, the pairing
//! semantics, and the descriptors' grouping.

use std::collections::BTreeMap;

use nodetool::behaviour::{self, drive, handoff, Behaviour, Input, Output};
use nodetool::compile::CompiledNode;
use nodetool::registry;
use nodetool::scalars;
use nodetool::Value;
use nodetool_fizzbuzz as _;

/// The behaviour of the named fizzbuzz node, stamped for the numeric
/// family member a compile-time resolution would have handed it — the same
/// way an engine builds it.
fn behaviour_of(type_ref: &str, member: &str) -> Box<dyn Behaviour> {
    let node_type = registry::node_type(type_ref).expect("the fizzbuzz plugin declares this node");
    let mut families = BTreeMap::new();
    families.insert(
        "numeric",
        registry::data_type(member).expect("a registered family member"),
    );
    let compiled = CompiledNode {
        node_type,
        label: node_type.label.to_owned(),
        parameters: BTreeMap::new(),
        families,
    };
    (node_type
        .behaviour
        .expect("the node type is declared through the authoring API"))(&compiled)
}

/// Drives the counter with three streamed inputs; returns its emissions
/// and its outcome.
async fn drive_counter(
    member: &'static str,
    start: Value,
    stop: Value,
    step: Value,
) -> (Vec<Value>, Result<(), behaviour::Error>) {
    let mut behaviour = behaviour_of("fizzbuzz/counter", member);
    let (start_tx, start_rx) = handoff();
    let (stop_tx, stop_rx) = handoff();
    let (step_tx, step_rx) = handoff();
    let (count_tx, mut count_rx) = handoff();
    let driven = tokio::spawn(async move {
        let mut inputs = [
            Input::new("start", start_rx),
            Input::new("stop", stop_rx),
            Input::new("step", step_rx),
        ];
        let mut outputs = [Output::new("count")];
        outputs[0].connect(count_tx, None);
        drive(behaviour.as_mut(), &mut inputs, &mut outputs).await
    });
    start_tx
        .send(start)
        .await
        .expect("the stream takes the value");
    stop_tx
        .send(stop)
        .await
        .expect("the stream takes the value");
    step_tx
        .send(step)
        .await
        .expect("the stream takes the value");
    drop(start_tx);
    drop(stop_tx);
    drop(step_tx);
    // The drain rides beside the drive, so a long sequence cannot stall on
    // a full hand-off.
    let mut emitted = Vec::new();
    let (outcome, ()) = tokio::join!(
        async { driven.await.expect("the node task ran to its end") },
        async {
            while let Some(value) = count_rx.recv().await {
                emitted.push(value);
            }
        }
    );
    (emitted, outcome)
}

#[tokio::test]
async fn the_counter_emits_start_through_stop_then_completes() {
    let (emitted, outcome) = drive_counter(
        "i32",
        Value::new(scalars::I32, 1),
        Value::new(scalars::I32, 5),
        Value::new(scalars::I32, 1),
    )
    .await;

    outcome.expect("the counter completed");
    let values: Vec<i32> = emitted
        .iter()
        .map(|value| *value.get::<i32>().expect("the resolved member's type"))
        .collect();
    assert_eq!(values, [1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn the_counter_includes_a_stop_the_accumulation_lands_exactly_on() {
    let (emitted, outcome) = drive_counter(
        "f64",
        Value::new(scalars::F64, 0.0),
        Value::new(scalars::F64, 1.0),
        Value::new(scalars::F64, 0.25),
    )
    .await;

    outcome.expect("the counter completed");
    let values: Vec<f64> = emitted
        .iter()
        .map(|value| *value.get::<f64>().expect("the resolved member's type"))
        .collect();
    assert_eq!(values, [0.0, 0.25, 0.5, 0.75, 1.0]);
}

#[tokio::test]
async fn an_inverted_range_yields_no_values_and_completes() {
    let (emitted, outcome) = drive_counter(
        "i32",
        Value::new(scalars::I32, 10),
        Value::new(scalars::I32, 1),
        Value::new(scalars::I32, 1),
    )
    .await;

    outcome.expect("the counter completed with an empty sequence");
    assert!(emitted.is_empty());
}

#[tokio::test]
async fn a_streamed_zero_step_is_a_behaviour_error() {
    let (emitted, outcome) = drive_counter(
        "i32",
        Value::new(scalars::I32, 1),
        Value::new(scalars::I32, 10),
        Value::new(scalars::I32, 0),
    )
    .await;

    let error = outcome.expect_err("a zero step never reaches a stop");
    assert!(error.to_string().contains("zero"), "{error}");
    assert!(emitted.is_empty());
}

#[tokio::test]
async fn a_later_arrival_does_not_restart_the_running_counter() {
    // A long sequence, so the node is still mid-run when fresh start, stop,
    // and step values arrive: they queue, the run finishes the sequence it
    // started, and the node completes — the sequence unchanged, no restart.
    let mut behaviour = behaviour_of("fizzbuzz/counter", "i32");
    let (start_tx, start_rx) = handoff();
    let (stop_tx, stop_rx) = handoff();
    let (step_tx, step_rx) = handoff();
    let (count_tx, mut count_rx) = handoff();
    let driven = tokio::spawn(async move {
        let mut inputs = [
            Input::new("start", start_rx),
            Input::new("stop", stop_rx),
            Input::new("step", step_rx),
        ];
        let mut outputs = [Output::new("count")];
        outputs[0].connect(count_tx, None);
        drive(behaviour.as_mut(), &mut inputs, &mut outputs).await
    });

    start_tx.send(Value::new(scalars::I32, 1)).await.unwrap();
    stop_tx.send(Value::new(scalars::I32, 100)).await.unwrap();
    step_tx.send(Value::new(scalars::I32, 1)).await.unwrap();

    let first = count_rx.recv().await.expect("the sequence began");
    assert_eq!(first.get::<i32>(), Some(&1));

    start_tx.send(Value::new(scalars::I32, 1)).await.unwrap();
    stop_tx.send(Value::new(scalars::I32, 1)).await.unwrap();
    step_tx.send(Value::new(scalars::I32, 1)).await.unwrap();
    drop(start_tx);
    drop(stop_tx);
    drop(step_tx);

    let mut values = vec![*first.get::<i32>().unwrap()];
    let (outcome, ()) = tokio::join!(async { driven.await.expect("the node task ran") }, async {
        while let Some(value) = count_rx.recv().await {
            values.push(*value.get::<i32>().unwrap());
        }
    });
    outcome.expect("the counter completed");
    assert_eq!(values, (1..=100).collect::<Vec<_>>());
}

/// Drives one condition with one value on each input; returns the boolean
/// it emitted.
async fn condition_result(
    type_ref: &'static str,
    member: &'static str,
    a: Value,
    b: Value,
) -> bool {
    let mut behaviour = behaviour_of(type_ref, member);
    let (a_tx, a_rx) = handoff();
    let (b_tx, b_rx) = handoff();
    let (result_tx, mut result_rx) = handoff();
    let driven = tokio::spawn(async move {
        let mut inputs = [Input::new("a", a_rx), Input::new("b", b_rx)];
        let mut outputs = [Output::new("result")];
        outputs[0].connect(result_tx, None);
        drive(behaviour.as_mut(), &mut inputs, &mut outputs).await
    });
    a_tx.send(a).await.expect("the stream takes the value");
    b_tx.send(b).await.expect("the stream takes the value");
    drop(a_tx);
    drop(b_tx);
    driven
        .await
        .expect("the node task ran")
        .expect("the condition completed");
    *result_rx
        .recv()
        .await
        .expect("the comparison emitted")
        .get::<bool>()
        .expect("the result is a bool")
}

#[tokio::test]
async fn each_operation_compares_across_several_numeric_types() {
    // Each operation once over i32, and again over members further along
    // the family: the same shared behaviour, stamped per resolved type.
    let cases: Vec<(&'static str, &'static str, Value, Value, bool)> = vec![
        (
            "fizzbuzz/eq",
            "i32",
            Value::new(scalars::I32, 3),
            Value::new(scalars::I32, 3),
            true,
        ),
        (
            "fizzbuzz/eq",
            "i32",
            Value::new(scalars::I32, 3),
            Value::new(scalars::I32, 4),
            false,
        ),
        (
            "fizzbuzz/ne",
            "i32",
            Value::new(scalars::I32, 3),
            Value::new(scalars::I32, 4),
            true,
        ),
        (
            "fizzbuzz/lt",
            "i32",
            Value::new(scalars::I32, 3),
            Value::new(scalars::I32, 4),
            true,
        ),
        (
            "fizzbuzz/le",
            "i32",
            Value::new(scalars::I32, 4),
            Value::new(scalars::I32, 4),
            true,
        ),
        (
            "fizzbuzz/gt",
            "i32",
            Value::new(scalars::I32, 3),
            Value::new(scalars::I32, 4),
            false,
        ),
        (
            "fizzbuzz/ge",
            "i32",
            Value::new(scalars::I32, 4),
            Value::new(scalars::I32, 4),
            true,
        ),
        (
            "fizzbuzz/eq",
            "f64",
            Value::new(scalars::F64, 1.5),
            Value::new(scalars::F64, 1.5),
            true,
        ),
        (
            "fizzbuzz/lt",
            "f64",
            Value::new(scalars::F64, 0.5),
            Value::new(scalars::F64, 1.5),
            true,
        ),
        (
            "fizzbuzz/ge",
            "u8",
            Value::new(scalars::U8, 200u8),
            Value::new(scalars::U8, 100u8),
            true,
        ),
        (
            "fizzbuzz/gt",
            "i16",
            Value::new(scalars::I16, -1i16),
            Value::new(scalars::I16, -2i16),
            true,
        ),
        (
            "fizzbuzz/le",
            "u64",
            Value::new(scalars::U64, 7u64),
            Value::new(scalars::U64, 9u64),
            true,
        ),
        (
            "fizzbuzz/ne",
            "i8",
            Value::new(scalars::I8, -3i8),
            Value::new(scalars::I8, -3i8),
            false,
        ),
    ];
    for (type_ref, member, a, b, expected) in cases {
        let result = condition_result(type_ref, member, a, b).await;
        assert_eq!(result, expected, "{type_ref} over {member}");
    }
}

#[tokio::test]
async fn each_arrival_fires_a_comparison_against_the_held_value() {
    // a=1, b=2, then a=3: three arrivals, three runs. The first pairs
    // (1, 2); the second re-fires the held a under the new condition — the
    // delivered 3 is a's current by then — pairing (3, 2); the third pairs
    // (3, 2) again. The node completes by the default rule when its inputs
    // end.
    let mut behaviour = behaviour_of("fizzbuzz/lt", "i32");
    let (a_tx, a_rx) = handoff();
    let (b_tx, b_rx) = handoff();
    let (result_tx, mut result_rx) = handoff();
    let driven = tokio::spawn(async move {
        let mut inputs = [Input::new("a", a_rx), Input::new("b", b_rx)];
        let mut outputs = [Output::new("result")];
        outputs[0].connect(result_tx, None);
        drive(behaviour.as_mut(), &mut inputs, &mut outputs).await
    });
    a_tx.send(Value::new(scalars::I32, 1)).await.unwrap();
    b_tx.send(Value::new(scalars::I32, 2)).await.unwrap();
    a_tx.send(Value::new(scalars::I32, 3)).await.unwrap();
    drop(a_tx);
    drop(b_tx);
    driven
        .await
        .expect("the node task ran")
        .expect("the condition completed");

    let mut results = Vec::new();
    while let Some(value) = result_rx.recv().await {
        results.push(*value.get::<bool>().unwrap());
    }
    assert_eq!(results, [true, false, false]);
}

#[test]
fn the_plugin_declares_one_counter_six_conditions_and_one_output() {
    let mut fizzbuzz: Vec<_> = registry::node_types()
        .filter(|node_type| node_type.plugin == "fizzbuzz")
        .map(|node_type| (node_type.type_ref, node_type.sub_group, node_type.label))
        .collect();
    fizzbuzz.sort_unstable();

    assert_eq!(
        fizzbuzz,
        vec![
            ("fizzbuzz/counter", None, "Counter"),
            ("fizzbuzz/eq", Some("condition"), "Equal"),
            ("fizzbuzz/ge", Some("condition"), "Greater or equal"),
            ("fizzbuzz/gt", Some("condition"), "Greater"),
            ("fizzbuzz/le", Some("condition"), "Less or equal"),
            ("fizzbuzz/lt", Some("condition"), "Less"),
            ("fizzbuzz/ne", Some("condition"), "Not equal"),
            ("fizzbuzz/output", None, "Output"),
        ]
    );
}
