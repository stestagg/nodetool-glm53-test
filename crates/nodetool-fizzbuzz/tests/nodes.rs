//! The fizzbuzz nodes' behaviours, driven one node at a time: the
//! counter's sequence — literal and streamed, inverted range, streamed
//! zero step, completion after exhaustion, no restart on later arrivals —
//! each operation of the two operator nodes across the numeric family,
//! their shared pairing rule, the arithmetic with no answer ending the run
//! reported, and the descriptors the palette lists.

use std::collections::{BTreeMap, BTreeSet};

use nodetool::behaviour::{
    self, drive, handoff, Behaviour, Input, Output, Sender, HANDOFF_CAPACITY,
};
use nodetool::compile::{CompiledNode, CompiledParameter};
use nodetool::registry;
use nodetool::scalars;
use nodetool::Value;
use nodetool_fizzbuzz as _;
use nodetool_fizzbuzz::numeric::{numeric_value, Numeric};

/// The behaviour of the named fizzbuzz node, stamped for the numeric
/// family member a compile-time resolution would have handed it and
/// carrying the choices a compiled instance holds — the same way an engine
/// builds it.
fn behaviour_of(type_ref: &str, member: &str, choices: &[(&str, &str)]) -> Box<dyn Behaviour> {
    let node_type = registry::node_type(type_ref).expect("the fizzbuzz plugin declares this node");
    let mut families = BTreeMap::new();
    families.insert(
        "numeric",
        registry::data_type(member).expect("a registered family member"),
    );
    let mut parameters = BTreeMap::new();
    for (name, option) in choices {
        let declared = node_type
            .choices
            .iter()
            .find(|choice| choice.name == *name)
            .expect("the node type declares this choice");
        assert!(
            declared.options.contains(option),
            "`{name}` does not offer `{option}`"
        );
        parameters.insert(
            declared.name,
            CompiledParameter {
                resolved_type: registry::data_type("String")
                    .expect("core ships the String scalar a choice's value is carried as"),
                value: Value::new(scalars::STRING, (*option).to_owned()),
            },
        );
    }
    let compiled = CompiledNode {
        node_type,
        label: node_type.label.to_owned(),
        parameters,
        families,
        fed: BTreeSet::new(),
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
    let mut behaviour = behaviour_of("fizzbuzz/counter", member, &[]);
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
    let mut behaviour = behaviour_of("fizzbuzz/counter", "i32", &[]);
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

/// One send in a scripted drive of an operator node: onto the gating
/// operand or onto the one it pairs against.
enum Operand {
    A(Value),
    B(Value),
}

/// Waits until the driver has taken a scripted value out of its hand-off,
/// so the arrival has landed before the next send: the driver polls its
/// inputs in declaration order, `a` ahead of `b`, and would otherwise
/// drain a whole buffered count stream past an operand sent between two of
/// its values — leaving a scripted interleaving to chance. A stream the
/// driver has stopped reading is closed, which is a landing too: the run
/// ended, and the outcome says so.
async fn landed(stream: &Sender<Value>) {
    for _ in 0..1000 {
        if stream.is_closed() || stream.capacity() == HANDOFF_CAPACITY {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the driver never took the scripted value out of its hand-off");
}

/// Drives one operator node — the arithmetic and the comparison share
/// their port shape — over a scripted interleaving of its two operands,
/// with the choice a compiled instance would carry. Returns what it
/// emitted, in order, and how the run ended. The drain rides beside the
/// drive, so no script stalls on a full hand-off.
async fn drive_operator(
    type_ref: &'static str,
    choice: (&'static str, &'static str),
    member: &'static str,
    script: Vec<Operand>,
) -> (Vec<Value>, Result<(), behaviour::Error>) {
    let mut behaviour = behaviour_of(type_ref, member, &[choice]);
    let (a_tx, a_rx) = handoff();
    let (b_tx, b_rx) = handoff();
    let (result_tx, mut result_rx) = handoff();
    let driven = tokio::spawn(async move {
        let mut inputs = [Input::new("a", a_rx), Input::new("b", b_rx)];
        let mut outputs = [Output::new("result")];
        outputs[0].connect(result_tx, None);
        drive(behaviour.as_mut(), &mut inputs, &mut outputs).await
    });
    let mut emitted = Vec::new();
    let (outcome, ()) = tokio::join!(
        async {
            for send in script {
                let (stream, value) = match send {
                    Operand::A(value) => (&a_tx, value),
                    Operand::B(value) => (&b_tx, value),
                };
                stream
                    .send(value)
                    .await
                    .expect("the stream takes the value");
                landed(stream).await;
            }
            drop(a_tx);
            drop(b_tx);
            driven.await.expect("the node task ran")
        },
        async {
            while let Some(value) = result_rx.recv().await {
                emitted.push(value);
            }
        }
    );
    (emitted, outcome)
}

/// The comparison over one pair: the operand it pairs against arrives as a
/// literal's stream does, once.
async fn compared<T: Numeric>(cmp: &'static str, a: T, b: T) -> bool {
    let (emitted, outcome) = drive_operator(
        "fizzbuzz/comparison",
        ("cmp", cmp),
        T::TYPE_REF,
        vec![Operand::A(numeric_value(a)), Operand::B(numeric_value(b))],
    )
    .await;
    outcome.expect("the comparison completed");
    assert_eq!(emitted.len(), 1, "one result per `a` arrival: {cmp}");
    *emitted[0].get::<bool>().expect("the result is a bool")
}

/// The arithmetic over a scripted count stream, the second operand held as
/// a literal's stream delivers it. Returns the results as the resolved
/// member and how the run ended.
async fn calculated<T: Numeric>(
    op: &'static str,
    counts: &[T],
    b: T,
) -> (Vec<T>, Result<(), behaviour::Error>) {
    let mut script = vec![Operand::B(numeric_value(b))];
    script.extend(counts.iter().map(|count| Operand::A(numeric_value(*count))));
    let (emitted, outcome) =
        drive_operator("fizzbuzz/arithmetic", ("op", op), T::TYPE_REF, script).await;
    let results = emitted
        .iter()
        .map(|value| *value.get::<T>().expect("the resolved member's type"))
        .collect();
    (results, outcome)
}

#[tokio::test]
async fn each_comparison_option_compares_across_several_numeric_types() {
    // Each option once over i32, and again over members further along the
    // family: one shared behaviour, the operation the instance's choice
    // named, stamped per resolved type.
    assert!(compared("equal", 3i32, 3i32).await);
    assert!(!compared("equal", 3i32, 4i32).await);
    assert!(compared("not equal", 3i32, 4i32).await);
    assert!(compared("less", 3i32, 4i32).await);
    assert!(compared("less or equal", 4i32, 4i32).await);
    assert!(!compared("greater", 3i32, 4i32).await);
    assert!(compared("greater or equal", 4i32, 4i32).await);
    assert!(compared("equal", 1.5f64, 1.5f64).await);
    assert!(compared("less", 0.5f64, 1.5f64).await);
    assert!(compared("greater or equal", 200u8, 100u8).await);
    assert!(compared("greater", -1i16, -2i16).await);
    assert!(compared("less or equal", 7u64, 9u64).await);
    assert!(!compared("not equal", -3i8, -3i8).await);
    assert!(compared("equal", 0.5f32, 0.5f32).await);
    assert!(compared("less", 7u16, 9u16).await);
    assert!(compared("greater", 9u32, 7u32).await);
    assert!(compared("not equal", 7i64, 9i64).await);
}

#[tokio::test]
async fn each_arithmetic_option_calculates_across_several_numeric_types() {
    assert_eq!(calculated("add", &[1i32, 2, 3], 10).await.0, [11, 12, 13]);
    assert_eq!(calculated("subtract", &[1i32, 2, 3], 1).await.0, [0, 1, 2]);
    assert_eq!(calculated("multiply", &[1i32, 2, 3], 3).await.0, [3, 6, 9]);
    assert_eq!(calculated("divide", &[10i32, 11, 12], 5).await.0, [2, 2, 2]);
    assert_eq!(calculated("modulo", &[3i32, 4, 5], 3).await.0, [0, 1, 2]);
    assert_eq!(calculated("add", &[1u8, 2], 3).await.0, [4, 5]);
    assert_eq!(calculated("multiply", &[2i8, 3], 4).await.0, [8, 12]);
    assert_eq!(
        calculated("divide", &[1.5f64, 3.0], 1.5).await.0,
        [1.0, 2.0]
    );
    assert_eq!(
        calculated("modulo", &[3.0f32, 4.5], 1.5).await.0,
        [0.0, 0.0]
    );
    assert_eq!(calculated("subtract", &[9u64, 8], 7).await.0, [2, 1]);
    assert_eq!(calculated("add", &[9u16, 8], 7).await.0, [16, 15]);
    assert_eq!(calculated("modulo", &[9u32, 8], 7).await.0, [2, 1]);
    assert_eq!(calculated("subtract", &[9i16, 8], 7).await.0, [2, 1]);
    assert_eq!(calculated("multiply", &[9i64, 8], 7).await.0, [63, 56]);
}

#[tokio::test]
async fn a_literal_second_operand_yields_exactly_one_result_per_count() {
    // The engine feeds a parameter literal as a stream that yields once,
    // and its delivery is an arrival like any other: both nodes answer the
    // `a` arrival alone, so the count of results is the count of counts —
    // the pairing a downstream consumer reads, whichever end the operand
    // arrives at.
    let counts: Vec<i32> = (1..=6).collect();
    let (results, outcome) = calculated("modulo", &counts, 3).await;
    outcome.expect("the arithmetic completed");
    assert_eq!(results, [1, 2, 0, 1, 2, 0]);

    // The same script with the operand arriving last: still one per count.
    let mut script: Vec<Operand> = counts
        .iter()
        .map(|count| Operand::A(numeric_value(*count)))
        .collect();
    script.push(Operand::B(numeric_value(3i32)));
    let (emitted, outcome) =
        drive_operator("fizzbuzz/arithmetic", ("op", "modulo"), "i32", script).await;
    outcome.expect("the arithmetic completed");
    assert_eq!(emitted.len(), counts.len());

    // And the comparison, the same rule: six counts, six booleans.
    let mut script = vec![Operand::B(numeric_value(0i32))];
    script.extend(
        counts
            .iter()
            .map(|count| Operand::A(numeric_value(count % 3))),
    );
    let (emitted, outcome) =
        drive_operator("fizzbuzz/comparison", ("cmp", "equal"), "i32", script).await;
    outcome.expect("the comparison completed");
    let flags: Vec<bool> = emitted
        .iter()
        .map(|value| *value.get::<bool>().expect("the result is a bool"))
        .collect();
    assert_eq!(flags, [false, false, true, false, false, true]);
}

#[tokio::test]
async fn a_streamed_second_operand_takes_effect_at_the_next_a_arrival() {
    // The operand's own arrivals emit nothing; each only updates the value
    // the next count pairs against.
    let (emitted, outcome) = drive_operator(
        "fizzbuzz/arithmetic",
        ("op", "add"),
        "i32",
        vec![
            Operand::B(numeric_value(10i32)),
            Operand::A(numeric_value(1i32)),
            Operand::B(numeric_value(20i32)),
            Operand::A(numeric_value(2i32)),
            Operand::A(numeric_value(3i32)),
        ],
    )
    .await;
    outcome.expect("the arithmetic completed");
    let results: Vec<i32> = emitted
        .iter()
        .map(|value| *value.get::<i32>().expect("the resolved member's type"))
        .collect();
    assert_eq!(results, [11, 22, 23]);

    let (emitted, outcome) = drive_operator(
        "fizzbuzz/comparison",
        ("cmp", "less"),
        "i32",
        vec![
            Operand::B(numeric_value(2i32)),
            Operand::A(numeric_value(1i32)),
            Operand::B(numeric_value(0i32)),
            Operand::A(numeric_value(1i32)),
        ],
    )
    .await;
    outcome.expect("the comparison completed");
    let flags: Vec<bool> = emitted
        .iter()
        .map(|value| *value.get::<bool>().expect("the result is a bool"))
        .collect();
    assert_eq!(flags, [true, false]);
}

#[tokio::test]
async fn an_arithmetic_with_no_answer_ends_the_run_as_a_reported_failure() {
    // A zero divisor and an overflow are the same kind of answer — one the
    // member does not have — and each ends the run reported, whatever
    // profile the build ran under: the tests pin the behaviour the product
    // ships, not `cargo test`'s debug overflow checks.
    let (emitted, outcome) = calculated("divide", &[5i32], 0).await;
    let error = outcome.expect_err("a zero divisor has no quotient");
    assert!(error.to_string().contains("the divisor is zero"), "{error}");
    assert!(emitted.is_empty());

    let (_, outcome) = calculated("modulo", &[5i32], 0).await;
    let error = outcome.expect_err("a zero divisor has no remainder");
    assert!(error.to_string().contains("the divisor is zero"), "{error}");

    let (_, outcome) = calculated("add", &[200u8], 100).await;
    let error = outcome.expect_err("200 + 100 is outside u8");
    assert!(
        error.to_string().contains("`add` of 200 and 100"),
        "{error}"
    );
    assert!(
        error.to_string().contains("the result is outside the type"),
        "{error}"
    );

    let (_, outcome) = calculated("subtract", &[1u8], 2).await;
    let error = outcome.expect_err("1 - 2 is outside u8");
    assert!(error.to_string().contains("outside the type"), "{error}");

    let (_, outcome) = calculated("multiply", &[100i8], 2).await;
    let error = outcome.expect_err("100 * 2 is outside i8");
    assert!(error.to_string().contains("outside the type"), "{error}");

    // The zero divisor is refused across the whole family, so one rule
    // answers for every member rather than an infinity here and a failure
    // there.
    let (results, outcome) = calculated("divide", &[1.0f64], 0.0).await;
    let error = outcome.expect_err("the zero divisor is refused across the family");
    assert!(error.to_string().contains("the divisor is zero"), "{error}");
    assert!(results.is_empty());
}

#[test]
fn the_plugin_declares_a_counter_two_operator_nodes_and_an_output() {
    let mut fizzbuzz: Vec<_> = registry::node_types()
        .filter(|node_type| node_type.plugin == "fizzbuzz")
        .map(|node_type| (node_type.type_ref, node_type.sub_group, node_type.label))
        .collect();
    fizzbuzz.sort_unstable();

    assert_eq!(
        fizzbuzz,
        vec![
            ("fizzbuzz/arithmetic", None, "Arithmetic"),
            ("fizzbuzz/comparison", None, "Comparison"),
            ("fizzbuzz/counter", None, "Counter"),
            ("fizzbuzz/output", None, "Output"),
        ]
    );
}

#[test]
fn the_operator_nodes_declare_their_operation_as_a_choice() {
    let choices = |type_ref: &str| {
        registry::node_type(type_ref)
            .expect("the fizzbuzz plugin declares this node")
            .choices
            .iter()
            .map(|choice| (choice.name, choice.options))
            .collect::<Vec<_>>()
    };

    assert_eq!(
        choices("fizzbuzz/arithmetic"),
        vec![(
            "op",
            &["add", "subtract", "multiply", "divide", "modulo"][..]
        )]
    );
    assert_eq!(
        choices("fizzbuzz/comparison"),
        vec![(
            "cmp",
            &[
                "equal",
                "not equal",
                "less",
                "less or equal",
                "greater",
                "greater or equal"
            ][..]
        )]
    );
    // Every other type of the crate declares no setting at all.
    assert!(choices("fizzbuzz/counter").is_empty());
    assert!(choices("fizzbuzz/output").is_empty());
}
