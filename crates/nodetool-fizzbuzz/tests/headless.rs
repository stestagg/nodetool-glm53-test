//! The headless binary's terminal behaviour: the shipped graph prints the
//! classic sequence, exactly one hundred lines — what its Output node
//! receives — while a graph with no Output node runs to completion in
//! silence, and a literal held on an Output node's input prints once. A
//! longer-range variant prints its first line long before the run could
//! end, and a reader that goes away early ends the run quietly. A missing
//! file, a load error, a compile error — a required input nothing carries
//! among them — and a run that ends in an error each end printed on the
//! error stream with a non-zero exit.

mod common;

use std::io::{BufRead, Read};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{fizzbuzz_line, long_counter};

const GRAPH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/graphs/fizzbuzz.yml");

/// The generous bound on the first line's arrival: the variant counted
/// below cannot finish inside it on any machine, so only a run printing as
/// values arrive can meet it.
const FIRST_LINE: Duration = Duration::from_secs(10);

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nodetool-fizzbuzz"))
}

#[test]
fn the_shipped_graph_prints_the_classic_sequence_as_its_only_output() {
    let output = binary().arg(GRAPH).output().expect("the binary runs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "nothing on the error stream");
    let lines: Vec<&str> = std::str::from_utf8(&output.stdout)
        .expect("stdout is text")
        .lines()
        .collect();
    // Exactly one hundred lines: what the Output node received. Every
    // other stream the graph carries — the count, the two flags, the
    // formatted number, the selections — prints nothing.
    let expected: Vec<String> = (1..=100).map(fizzbuzz_line).collect();
    assert_eq!(lines, expected);
}

#[test]
fn a_longer_range_prints_its_first_line_long_before_the_run_could_end() {
    let path = long_counter("1000000000");

    let mut child = binary()
        .arg(&path)
        .stdout(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    let mut lines = std::io::BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
    // Read the first line on a thread the deadline can outlive: a run
    // printing as values arrive delivers in milliseconds, while one that
    // dumps at its end arrives only after the whole count — a billion
    // counts cannot finish inside the bound.
    let (line_tx, line_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = line_tx.send(lines.next());
    });
    let first = line_rx.recv_timeout(FIRST_LINE);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&path);

    let line = first
        .expect("the first line arrives inside the bound: the run prints as values arrive, it does not dump at its end")
        .expect("the child's stdout is readable")
        .expect("the stream opens with a line");
    assert_eq!(line, "1");
}

#[test]
fn a_graph_with_no_output_node_prints_nothing_and_still_runs_to_completion() {
    // A bare counter: fifty values with nowhere to go. Printing is keyed
    // to the Output node, so this run is silent and correct — not a graph
    // whose unwired streams leak to the terminal.
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-count.yml");
    std::fs::write(
        &path,
        concat!(
            "schema_version: 1\n",
            "name: bare counter\n",
            "nodes:\n",
            "  - uuid: 00000000-0000-0000-0000-300000000003\n",
            "    type_ref: fizzbuzz/counter\n",
            "    label: counter\n",
            "    parameters:\n",
            "      start: 1\n",
            "      stop: 5\n",
            "      step: 1\n",
            "edges: []\n",
        ),
    )
    .expect("the file is written");

    let output = binary().arg(&path).output().expect("the binary runs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "nothing on the error stream");
    assert!(output.stdout.is_empty(), "no Output node, no stdout");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_literal_held_on_an_output_nodes_input_prints_once() {
    // A parameter literal is a value the input receives like any other:
    // the stream that yields once and completes prints its one line.
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-held-output.yml");
    std::fs::write(
        &path,
        concat!(
            "schema_version: 1\n",
            "name: a held greeting\n",
            "nodes:\n",
            "  - uuid: 00000000-0000-0000-0000-300000000005\n",
            "    type_ref: fizzbuzz/output\n",
            "    label: output\n",
            "    parameters:\n",
            "      text: hello\n",
            "edges: []\n",
        ),
    )
    .expect("the file is written");

    let output = binary().arg(&path).output().expect("the binary runs");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello\n");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_reader_closing_early_ends_the_run_quietly_not_as_a_failure() {
    // The truncation idiom — `nodetool-fizzbuzz long.yml | head -5`: the
    // reader goes away and the run ends with it, exit 0, no panic and no
    // backtrace on the error stream.
    let path = long_counter("10000000");
    let mut child = binary()
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    let mut lines = std::io::BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
    let first: Vec<String> = lines
        .by_ref()
        .take(5)
        .map(|line| line.expect("a line arrived"))
        .collect();
    assert_eq!(first, ["1", "2", "Fizz", "4", "Buzz"]);
    drop(lines);
    let status = child.wait().expect("the run ends");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut stderr)
        .expect("stderr is readable");
    let _ = std::fs::remove_file(&path);

    assert!(status.success(), "a reader gone is not a failure: {status}");
    assert!(
        stderr.is_empty(),
        "no panic and no backtrace reaches the terminal: {stderr}"
    );
}

#[test]
fn a_required_input_nothing_carries_fails_to_compile_naming_both() {
    // The shipped counter minus its step: the file loads, and the compile
    // must refuse it — naming the node and the input — instead of a run
    // that stalls silent. The wait is bounded, so a regression to the hang
    // fails rather than stalls the suite.
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-missing-step.yml");
    std::fs::write(
        &path,
        concat!(
            "schema_version: 1\n",
            "name: missing step\n",
            "nodes:\n",
            "  - uuid: 00000000-0000-0000-0000-300000000004\n",
            "    type_ref: fizzbuzz/counter\n",
            "    label: counter\n",
            "    parameters:\n",
            "      start: 1\n",
            "      stop: 10\n",
            "edges: []\n",
        ),
    )
    .expect("the file is written");

    let mut child = binary()
        .arg(&path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    let started = Instant::now();
    let status = loop {
        match child.try_wait().expect("the exit status is readable") {
            Some(status) => break status,
            None => {
                assert!(
                    started.elapsed() < FIRST_LINE,
                    "the run must not wait on an input nothing carries"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    };
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("stdout is piped")
        .read_to_string(&mut stdout)
        .expect("stdout is readable");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut stderr)
        .expect("stderr is readable");
    let _ = std::fs::remove_file(&path);

    assert!(!status.success());
    assert!(stdout.is_empty());
    assert!(stderr.contains("compile error"), "{stderr}");
    assert!(stderr.contains("counter"), "{stderr}");
    assert!(stderr.contains("`step`"), "{stderr}");
}

#[test]
fn a_missing_file_ends_non_zero_naming_the_path() {
    let output = binary()
        .arg("/no/such/graph.yml")
        .output()
        .expect("the binary runs");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("/no/such/graph.yml"), "{stderr}");
}

#[test]
fn a_load_error_ends_non_zero_naming_what_and_where() {
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-broken.yml");
    std::fs::write(
        &path,
        "schema_version: 1\nnodes: []\nedges: []\nsurprise: true\n",
    )
    .expect("the file is written");

    let output = binary().arg(&path).output().expect("the binary runs");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("load error"), "{stderr}");
    assert!(stderr.contains("unknown field `surprise`"), "{stderr}");
    assert!(stderr.contains("document.surprise"), "{stderr}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_compile_error_ends_non_zero_naming_the_type() {
    // This binary carries only the plugins its graphs need, and the
    // compiler refuses a type no linked plugin declares.
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-uncompilable.yml");
    std::fs::write(
        &path,
        "schema_version: 1\nnodes:\n  - uuid: 00000000-0000-0000-0000-200000000001\n    type_ref: nowhere/nothing\nedges: []\n",
    )
    .expect("the file is written");

    let output = binary().arg(&path).output().expect("the binary runs");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("compile error"), "{stderr}");
    assert!(stderr.contains("nowhere/nothing"), "{stderr}");
    assert!(stderr.contains("no linked plugin declares"), "{stderr}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_run_that_ends_in_an_error_exits_non_zero_naming_the_node() {
    // A counter feeds another counter's `step` a streamed zero: the graph
    // compiles clean, the run ends fail-fast on the sequence's first firing.
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-failing.yml");
    std::fs::write(
        &path,
        concat!(
            "schema_version: 1\n",
            "name: streamed zero step\n",
            "nodes:\n",
            "  - uuid: 00000000-0000-0000-0000-300000000001\n",
            "    type_ref: fizzbuzz/counter\n",
            "    label: zero maker\n",
            "    parameters:\n",
            "      start: 0\n",
            "      stop: 0\n",
            "      step: 1\n",
            "  - uuid: 00000000-0000-0000-0000-300000000002\n",
            "    type_ref: fizzbuzz/counter\n",
            "    label: stepper\n",
            "    parameters:\n",
            "      start: 1\n",
            "      stop: 10\n",
            "edges:\n",
            "  - from: 00000000-0000-0000-0000-300000000001\n",
            "    from_port: count\n",
            "    to: 00000000-0000-0000-0000-300000000002\n",
            "    to_port: step\n",
        ),
    )
    .expect("the file is written");

    let output = binary().arg(&path).output().expect("the binary runs");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("the run ended in failure"), "{stderr}");
    assert!(stderr.contains("stepper"), "{stderr}");
    assert!(
        !stderr.contains("00000000-0000-0000-0000-300000000002"),
        "the terminal names the node the user labelled, not its uuid: {stderr}"
    );
    assert!(stderr.contains("zero step"), "{stderr}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_graph_path_is_required() {
    let output = binary().output().expect("the binary runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage"), "{stderr}");
}
