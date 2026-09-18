//! The stream semantics, proven one node at a time: per-arrival triggering
//! with held-value pairing, trigger serialisation, first-value gating, held
//! values surviving their input's end, completion in its default and early
//! forms, self-driving sources, degenerate inputs, fan-out, backpressure,
//! error propagation, and the one value representation carrying a
//! union-declared port's types.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nodetool::async_trait;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use nodetool::behaviour::{
    drive, handoff, Behaviour, Error, Flow, Input, Io, Output, Trigger, HANDOFF_CAPACITY,
};
use nodetool::registry;
use nodetool::scalars;
use nodetool::Value;
use test_plugin_gamma as _;

/// The runs a test behaviour recorded, readable while the node is driven.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn record(&self, line: String) {
        self.0
            .lock()
            .expect("the log lock is never poisoned")
            .push(line);
    }

    fn runs(&self) -> Vec<String> {
        self.0
            .lock()
            .expect("the log lock is never poisoned")
            .clone()
    }
}

/// The trigger name a behaviour sees: an input's name, or the node's start.
fn trigger_name(trigger: &Trigger) -> &'static str {
    match trigger {
        Trigger::Start => "start",
        Trigger::Arrival(name) => name,
    }
}

fn int(value: i32) -> Value {
    Value::new(scalars::I32, value)
}

fn current_i32(io: &mut Io<'_>, name: &str) -> i32 {
    io.input(name)
        .current()
        .and_then(|value| value.get::<i32>().copied())
        .expect("the run is gated on this input's first value")
}

/// An input wired to a sender the test feeds.
fn fed(name: &'static str) -> (mpsc::Sender<Value>, Input) {
    let (tx, rx) = handoff();
    (tx, Input::new(name, rx))
}

/// An output wired to one receiver the test reads.
fn collected(name: &'static str) -> (Output, mpsc::Receiver<Value>) {
    let (tx, rx) = handoff();
    let mut output = Output::new(name);
    output.connect(tx, None);
    (output, rx)
}

/// Two integer inputs; each run records the pairing it saw.
struct Pairer {
    log: Log,
}

#[async_trait]
impl Behaviour for Pairer {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let a = current_i32(io, "a");
        let b = current_i32(io, "b");
        self.log
            .record(format!("{} a={a} b={b}", trigger_name(&trigger)));
        Ok(Flow::Continue)
    }
}

/// Two integer inputs; the first run blocks on a signal — reading its
/// pairing when it starts, recording it once released — and later runs
/// record as they go.
struct Blocking {
    log: Log,
    started: Option<oneshot::Sender<()>>,
    release: Option<oneshot::Receiver<()>>,
}

#[async_trait]
impl Behaviour for Blocking {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let pairing = format!(
            "{} a={} b={}",
            trigger_name(&trigger),
            current_i32(io, "a"),
            current_i32(io, "b")
        );
        if let Some(started) = self.started.take() {
            started.send(()).expect("the test waits for the first run");
        }
        if let Some(release) = self.release.take() {
            release.await.expect("the test releases the first run");
        }
        self.log.record(pairing);
        Ok(Flow::Continue)
    }
}

/// One integer input, one integer output: emits the input's current value,
/// then either carries on or completes by its logic.
struct Echo {
    complete: bool,
}

#[async_trait]
impl Behaviour for Echo {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let value = current_i32(io, "a");
        io.output("out").emit(int(value)).await;
        Ok(if self.complete {
            Flow::Complete
        } else {
            Flow::Continue
        })
    }
}

/// A source with no inputs: emits its words once, on its own schedule, then
/// its logic is done.
struct Words {
    words: Vec<&'static str>,
}

#[async_trait]
impl Behaviour for Words {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        for word in self.words.drain(..) {
            io.output("out")
                .emit(Value::new(scalars::STRING, word.to_owned()))
                .await;
        }
        Ok(Flow::Complete)
    }
}

/// A source that floods the hand-off: flags when its last emission crossed.
struct Burst {
    emitted: Arc<AtomicBool>,
}

#[async_trait]
impl Behaviour for Burst {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        for value in 0..=(HANDOFF_CAPACITY as i32) {
            io.output("out").emit(int(value)).await;
        }
        self.emitted.store(true, Ordering::SeqCst);
        Ok(Flow::Complete)
    }
}

/// Raises, the way errors must: nothing swallowed, nothing skipped.
struct Failer;

#[async_trait]
impl Behaviour for Failer {
    async fn process(&mut self, _trigger: Trigger, _io: &mut Io<'_>) -> Result<Flow, Error> {
        Err("the behaviour failed".into())
    }
}

/// A union-declared input: the same behaviour reads whichever declared type
/// the value carries.
struct UnionReader {
    log: Log,
}

#[async_trait]
impl Behaviour for UnionReader {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let value = io
            .input("n")
            .current()
            .expect("the run is gated on this input's first value");
        if let Some(&v) = value.get::<i32>() {
            self.log.record(format!("i32={v}"));
        } else if let Some(&v) = value.get::<f64>() {
            self.log.record(format!("f64={v}"));
        } else {
            panic!("a type the port did not declare");
        }
        Ok(Flow::Continue)
    }
}

/// A node whose real arrival semantics are its own: each run on `a`
/// consumes `b`'s arrivals directly, pairing until that stream ends.
struct Zipper {
    log: Log,
}

#[async_trait]
impl Behaviour for Zipper {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        self.log
            .record(format!("run on {}", trigger_name(&trigger)));
        if trigger_name(&trigger) != "a" {
            return Ok(Flow::Continue);
        }
        let a = current_i32(io, "a");
        let sums = {
            let b = io.input("b");
            let mut sums = Vec::new();
            while let Some(value) = b.next().await {
                sums.push(a + value.get::<i32>().copied().expect("b is declared i32"));
            }
            sums
        };
        for sum in sums {
            io.output("out").emit(int(sum)).await;
        }
        Ok(Flow::Continue)
    }
}

#[tokio::test]
async fn an_arrival_pairs_against_the_held_value_of_every_other_input() {
    let log = Log::default();
    let (a_tx, a) = fed("a");
    let (b_tx, b) = fed("b");
    let mut behaviour = Pairer { log: log.clone() };
    let mut inputs = [a, b];
    let mut outputs = [];

    a_tx.send(int(1)).await.expect("the hand-off takes it");
    b_tx.send(int(2)).await.expect("the hand-off takes it");
    a_tx.send(int(3)).await.expect("the hand-off takes it");
    drop(a_tx);
    drop(b_tx);

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("the node completes");

    assert_eq!(
        log.runs(),
        ["a a=1 b=2", "a a=3 b=2", "b a=3 b=2"],
        "each arrival fires its own run, pairing against the held values of its moment"
    );
}

#[tokio::test]
async fn arrivals_landing_during_a_run_fire_serially_in_arrival_order() {
    let log = Log::default();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let (a_tx, a) = fed("a");
    let (b_tx, b) = fed("b");
    let mut behaviour = Blocking {
        log: log.clone(),
        started: Some(started_tx),
        release: Some(release_rx),
    };
    let run = tokio::spawn(async move {
        let mut inputs = [a, b];
        let mut outputs = [];
        drive(&mut behaviour, &mut inputs, &mut outputs).await
    });

    a_tx.send(int(1)).await.expect("the hand-off takes it");
    b_tx.send(int(10)).await.expect("the hand-off takes it");
    started_rx.await.expect("the first run started");

    // Values landing while the first run is still in flight: two further
    // arrivals on `a`, one on `b`. None of them may touch the held slots
    // until the in-flight run is done — each run pairs the held values as
    // of its own start.
    a_tx.send(int(2)).await.expect("the hand-off takes it");
    a_tx.send(int(3)).await.expect("the hand-off takes it");
    b_tx.send(int(20)).await.expect("the hand-off takes it");
    drop(a_tx);
    drop(b_tx);
    release_tx.send(()).expect("the test releases the run");

    run.await.expect("the node task ran").expect("it completes");
    assert_eq!(
        log.runs(),
        [
            "a a=1 b=10", // the in-flight run: `b` still holds 10, 20 undelivered
            "b a=1 b=10", // the gate-opening arrival's own run
            "a a=2 b=10", // queued runs pair the held values of their own start
            "a a=3 b=10",
            "b a=3 b=20", // only a run firing after 20's delivery pairs it
        ],
        "arrivals landing during a run wait in the hand-off, then fire serially in arrival order, each pairing the held values as of its own start"
    );
}

#[tokio::test(start_paused = true)]
async fn runs_wait_until_every_input_has_delivered_a_first_value() {
    let log = Log::default();
    let (a_tx, a) = fed("a");
    let (b_tx, b) = fed("b");
    let mut behaviour = Pairer { log: log.clone() };
    let run = tokio::spawn(async move {
        let mut inputs = [a, b];
        let mut outputs = [];
        drive(&mut behaviour, &mut inputs, &mut outputs).await
    });

    for value in 1..=3 {
        a_tx.send(int(value)).await.expect("the hand-off takes it");
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        log.runs().is_empty(),
        "no run fires while an input has delivered nothing"
    );

    b_tx.send(int(10)).await.expect("the hand-off takes it");
    drop(a_tx);
    drop(b_tx);
    run.await.expect("the node task ran").expect("it completes");

    assert_eq!(
        log.runs(),
        ["a a=1 b=10", "a a=2 b=10", "a a=3 b=10", "b a=3 b=10"],
        "the queued arrivals fired in order once the gate opened"
    );
}

#[tokio::test]
async fn a_held_value_survives_its_input_completing() {
    let log = Log::default();
    let (a_tx, a) = fed("a");
    let (b_tx, b) = fed("b");
    let mut behaviour = Pairer { log: log.clone() };
    let mut inputs = [a, b];
    let mut outputs = [];

    // The constant case: a stream that yields once and completes.
    b_tx.send(int(7)).await.expect("the hand-off takes it");
    drop(b_tx);
    a_tx.send(int(1)).await.expect("the hand-off takes it");
    a_tx.send(int(2)).await.expect("the hand-off takes it");
    drop(a_tx);

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("the node completes");

    assert_eq!(
        log.runs(),
        ["a a=1 b=7", "a a=2 b=7", "b a=2 b=7"],
        "every later arrival still pairs against the held value"
    );
}

#[tokio::test]
async fn a_node_completes_once_every_arrived_value_is_processed() {
    let (tx, a) = fed("a");
    let (output, mut out_rx) = collected("out");
    let mut behaviour = Echo { complete: false };
    let mut inputs = [a];
    let mut outputs = [output];

    tx.send(int(1)).await.expect("the hand-off takes it");
    tx.send(int(2)).await.expect("the hand-off takes it");
    tx.send(int(3)).await.expect("the hand-off takes it");
    drop(tx);

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("every input complete and every arrived value processed: the node completes");

    let mut emitted = Vec::new();
    while let Some(value) = out_rx.recv().await {
        emitted.push(value.get::<i32>().copied().expect("i32").to_string());
    }
    assert_eq!(emitted, ["1", "2", "3"], "each value emitted as it arrived");
    assert!(
        out_rx.recv().await.is_none(),
        "on completion the node's outputs end"
    );
}

#[tokio::test]
async fn a_node_may_complete_earlier_when_its_logic_is_done() {
    let (tx, a) = fed("a");
    let (output, mut out_rx) = collected("out");
    let mut behaviour = Echo { complete: true };
    let mut inputs = [a];
    let mut outputs = [output];

    for value in 1..=3 {
        tx.send(int(value)).await.expect("the hand-off takes it");
    }
    drop(tx);

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("the node's logic ended the node");

    let mut emitted = Vec::new();
    while let Some(value) = out_rx.recv().await {
        emitted.push(value.get::<i32>().copied().expect("i32").to_string());
    }
    assert_eq!(emitted, ["1"], "the first run emitted, then the node ended");
    assert!(out_rx.recv().await.is_none(), "its outputs ended with it");
}

#[tokio::test]
async fn a_source_drives_itself_and_completes_when_exhausted() {
    let (output, mut out_rx) = collected("out");
    let mut behaviour = Words {
        words: vec!["alpha", "beta", "gamma"],
    };
    let mut outputs = [output];

    drive(&mut behaviour, &mut [], &mut outputs)
        .await
        .expect("the source completes after its last value");

    let mut emitted = Vec::new();
    while let Some(value) = out_rx.recv().await {
        emitted.push(value.get::<String>().expect("String").to_owned());
    }
    assert_eq!(emitted, ["alpha", "beta", "gamma"]);
    assert!(out_rx.recv().await.is_none(), "its outputs ended");
}

#[tokio::test]
async fn a_node_whose_inputs_are_all_degenerate_never_fires_and_completes() {
    let log = Log::default();
    let mut behaviour = Pairer { log: log.clone() };
    let mut inputs = [Input::unconnected("a"), Input::unconnected("b")];
    let mut outputs = [];

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("nothing can ever arrive: the node completes at once");

    assert!(
        log.runs().is_empty(),
        "the degenerate inputs never fired a run"
    );
}

#[tokio::test]
async fn one_emission_reaches_every_connected_downstream_input() {
    let (tx, a) = fed("a");
    let (mut output, mut first_rx) = collected("out");
    let (second_tx, mut second_rx) = handoff();
    output.connect(second_tx, None);

    let mut behaviour = Echo { complete: false };
    let mut inputs = [a];
    let mut outputs = [output];

    tx.send(int(9)).await.expect("the hand-off takes it");
    drop(tx);

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("the node completes");

    for rx in [&mut first_rx, &mut second_rx] {
        let value = rx.recv().await.expect("each downstream got the emission");
        assert_eq!(value.get::<i32>(), Some(&9));
        assert!(rx.recv().await.is_none(), "and the stream's end");
    }
}

#[tokio::test(start_paused = true)]
async fn a_producer_waits_while_its_consumer_lags_behind_the_hand_off() {
    let emitted = Arc::new(AtomicBool::new(false));
    let (output, mut out_rx) = collected("out");
    let mut behaviour = Burst {
        emitted: emitted.clone(),
    };
    let run = tokio::spawn(async move {
        let mut outputs = [output];
        drive(&mut behaviour, &mut [], &mut outputs).await
    });

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        !emitted.load(Ordering::SeqCst),
        "the last emission waits: the hand-off is full and the consumer is stalled"
    );

    let first = out_rx.recv().await.expect("the first value crossed");
    assert_eq!(first.get::<i32>(), Some(&0));
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        emitted.load(Ordering::SeqCst),
        "one value received freed the producer to finish"
    );

    let mut values = vec![0];
    while let Some(value) = out_rx.recv().await {
        values.push(value.get::<i32>().copied().expect("i32"));
    }
    assert_eq!(
        values,
        (0..=(HANDOFF_CAPACITY as i32)).collect::<Vec<_>>(),
        "every value crossed, in order, once the consumer kept up"
    );
    run.await.expect("the node task ran").expect("it completes");
}

#[tokio::test]
async fn a_behaviour_error_is_raised_never_swallowed() {
    let (tx, a) = fed("a");
    let mut behaviour = Failer;
    let mut inputs = [a];
    let mut outputs = [];

    tx.send(int(1)).await.expect("the hand-off takes it");
    drop(tx);

    let error = drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect_err("the error arrives on the run path");
    assert!(
        error.to_string().contains("the behaviour failed"),
        "the failure is named: {error}"
    );
}

#[tokio::test]
async fn a_union_declared_port_receives_every_declared_type_through_one_representation() {
    for (values, expected) in [
        (vec![int(1), int(2)], vec!["i32=1", "i32=2"]),
        (vec![Value::new(scalars::F64, 1.5f64)], vec!["f64=1.5"]),
    ] {
        let log = Log::default();
        let (tx, n) = fed("n");
        let mut behaviour = UnionReader { log: log.clone() };
        let mut inputs = [n];
        let mut outputs = [];
        for value in values {
            tx.send(value).await.expect("the hand-off takes it");
        }
        drop(tx);
        drive(&mut behaviour, &mut inputs, &mut outputs)
            .await
            .expect("the node completes");
        assert_eq!(log.runs(), expected, "the same behaviour read each type");
    }
}

#[tokio::test]
async fn a_behaviour_consuming_arrivals_directly_uses_the_same_faces() {
    let log = Log::default();
    let (a_tx, a) = fed("a");
    let (b_tx, b) = fed("b");
    let (output, mut out_rx) = collected("out");
    let mut behaviour = Zipper { log: log.clone() };
    let mut inputs = [a, b];
    let mut outputs = [output];

    a_tx.send(int(10)).await.expect("the hand-off takes it");
    for value in 1..=3 {
        b_tx.send(int(value)).await.expect("the hand-off takes it");
    }
    drop(b_tx);
    a_tx.send(int(20)).await.expect("the hand-off takes it");
    drop(a_tx);

    drive(&mut behaviour, &mut inputs, &mut outputs)
        .await
        .expect("the node completes");

    assert_eq!(
        log.runs(),
        ["run on a", "run on a", "run on b"],
        "only the arrivals the driver delivers fire runs; the ones the behaviour consumed directly do not"
    );
    let mut emitted = Vec::new();
    while let Some(value) = out_rx.recv().await {
        emitted.push(value.get::<i32>().copied().expect("i32"));
    }
    assert_eq!(
        emitted,
        [12, 13],
        "the direct consumption paired each consumed arrival"
    );
}

#[tokio::test(start_paused = true)]
async fn a_node_holding_a_degenerate_input_never_fires_nor_completes() {
    let (tx, a) = fed("a");
    let b = Input::unconnected("b");
    let mut behaviour = Pairer {
        log: Log::default(),
    };
    let mut inputs = [a, b];
    let mut outputs = [];

    tx.send(int(1)).await.expect("the hand-off takes it");
    drop(tx);

    let outcome = tokio::time::timeout(Duration::from_millis(50), async {
        drive(&mut behaviour, &mut inputs, &mut outputs).await
    })
    .await;
    let Err(_elapsed) = &outcome else {
        panic!("the node completed or its behaviour failed; the run did not hang: {outcome:?}")
    };
}

#[tokio::test(start_paused = true)]
async fn a_never_delivering_input_stalls_its_upstream_instead_of_buffering_its_stream() {
    let (tx, a) = fed("a");
    let b = Input::unconnected("b");
    let log = Log::default();
    let mut behaviour = Pairer { log: log.clone() };
    let mut inputs = [a, b];
    let mut outputs = [];
    let run = tokio::spawn(async move { drive(&mut behaviour, &mut inputs, &mut outputs).await });

    // Far more than the hand-offs can hold: per the settlement, the feeder
    // stalls once `a`'s hand-off fills — the driver holds one hand-off's
    // worth unprocessed, never the whole stream.
    let taken = Arc::new(AtomicUsize::new(0));
    let feeder = {
        let taken = taken.clone();
        tokio::spawn(async move {
            for value in 0..10_000 {
                if tx.send(int(value)).await.is_err() {
                    break;
                }
                taken.fetch_add(1, Ordering::SeqCst);
            }
        })
    };

    tokio::time::sleep(Duration::from_millis(10)).await;
    let taken = taken.load(Ordering::SeqCst);
    assert!(
        taken <= 2 * HANDOFF_CAPACITY,
        "the driver holds a bounded backlog, not the stream: {taken} values taken"
    );
    assert!(
        taken < 10_000,
        "the feeder stalled on its full hand-off: {taken} values taken"
    );
    assert!(
        log.runs().is_empty(),
        "the gate never opened, so no run ever fired"
    );

    let outcome = tokio::time::timeout(Duration::from_millis(50), run).await;
    let Err(_elapsed) = &outcome else {
        panic!("the node completed or its behaviour failed; the run did not hang: {outcome:?}")
    };
    feeder.abort();
}

#[tokio::test(start_paused = true)]
async fn a_backlogged_stream_resumes_once_the_gate_opens_and_every_arrival_fires() {
    let (tx, a) = fed("a");
    let (b_tx, b) = fed("b");
    let log = Log::default();
    let mut behaviour = Pairer { log: log.clone() };
    let mut inputs = [a, b];
    let mut outputs = [];
    let run = tokio::spawn(async move { drive(&mut behaviour, &mut inputs, &mut outputs).await });

    // The other half of the backlog mechanism. While `b` still owes its
    // first value, `a` floods more than the hand-offs can hold: the backlog
    // fills, the driver stops polling `a`, its hand-off fills, and the
    // feeder stalls — one hand-off's worth in the channel, one unprocessed.
    let taken = Arc::new(AtomicUsize::new(0));
    tokio::spawn({
        let taken = taken.clone();
        async move {
            for value in 0..10_000 {
                if tx.send(int(value)).await.is_err() {
                    break;
                }
                taken.fetch_add(1, Ordering::SeqCst);
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;
    assert!(
        log.runs().is_empty(),
        "the gate is still closed: no run has fired"
    );
    assert_eq!(
        taken.load(Ordering::SeqCst),
        2 * HANDOFF_CAPACITY,
        "the backlog filled: one hand-off's worth in the channel, one unprocessed"
    );

    // The gate opens into a full backlog. The queued runs must drain it:
    // polling resumes, the stalled feeder finishes, every arrival fires.
    // A driver that never resumed would hang here with `b` delivered and
    // `a`'s tail unpolled — the timeout below turns that into a failure.
    b_tx.send(int(1)).await.expect("the hand-off takes it");
    drop(b_tx);

    let outcome = tokio::time::timeout(Duration::from_millis(50), run)
        .await
        .expect("the node completed: the backlog drained once the gate opened");
    outcome
        .expect("the node task ran")
        .expect("the node completes without an error");

    let runs = log.runs();
    assert_eq!(
        runs.len(),
        10_001,
        "every `a` arrival fired its run, plus `b`'s own"
    );
    assert_eq!(
        runs[0], "a a=0 b=1",
        "the queued arrivals fired once the gate opened, pairing `b`'s held value"
    );
    assert_eq!(
        runs[10_000], "a a=9999 b=1",
        "the stream's tail fired too: polling resumed as the backlog drained"
    );
}

#[tokio::test]
async fn the_registry_hands_the_declared_behaviour_to_the_driver() {
    let node_type = registry::node_type("gamma/doubler").expect("declared by test plugin gamma");
    let compiled = nodetool::compile::CompiledNode {
        node_type,
        label: node_type.label.to_owned(),
        parameters: Default::default(),
        families: Default::default(),
        fed: Default::default(),
    };
    let mut behaviour = (node_type
        .behaviour
        .expect("declared through the authoring API"))(&compiled);

    let (tx, value) = fed("value");
    let (output, mut out_rx) = collected("value");
    let mut inputs = [value];
    let mut outputs = [output];

    tx.send(int(21)).await.expect("the hand-off takes it");
    drop(tx);

    drive(behaviour.as_mut(), &mut inputs, &mut outputs)
        .await
        .expect("the node completes");

    let emitted = out_rx.recv().await.expect("the emission crossed");
    assert_eq!(emitted.get::<i32>(), Some(&42));
    assert!(out_rx.recv().await.is_none(), "the outputs ended");
}

#[test]
fn a_descriptor_without_behaviour_is_still_a_whole_declaration() {
    let node_type =
        registry::node_type("gamma/passthrough").expect("declared by test plugin gamma");
    assert!(node_type.behaviour.is_none());
}
