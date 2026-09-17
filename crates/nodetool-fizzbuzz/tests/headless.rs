//! The headless binary's terminal behavior: the shipped graph prints the
//! classic sequence, exactly one hundred lines — the case selection's
//! output, the one stream the graph leaves unconnected — and a
//! longer-range variant streams its lines while the run is still going. A
//! missing file, a load error, a compile error, and a run that ends in an
//! error each end printed on the error stream with a non-zero exit.

use std::io::BufRead;
use std::process::{Command, Stdio};

const GRAPH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/graphs/fizzbuzz.yml");

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nodetool-fizzbuzz"))
}

fn fizzbuzz_line(count: i32) -> String {
    match (count % 3 == 0, count % 5 == 0) {
        (true, true) => "FizzBuzz".to_owned(),
        (true, false) => "Fizz".to_owned(),
        (false, true) => "Buzz".to_owned(),
        (false, false) => count.to_string(),
    }
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
    // Exactly one hundred lines: the connected streams — the counter's
    // count, the two conditions' booleans — print nothing.
    let expected: Vec<String> = (1..=100).map(fizzbuzz_line).collect();
    assert_eq!(lines, expected);
}

#[test]
fn a_longer_range_streams_its_lines_while_the_run_is_still_going() {
    let hand_written = std::fs::read_to_string(GRAPH)
        .expect("the shipped graph is readable")
        .replace("stop: 100", "stop: 10000000");
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-streaming.yml");
    std::fs::write(&path, hand_written).expect("the variant is written");

    let mut child = binary()
        .arg(&path)
        .stdout(Stdio::piped())
        .spawn()
        .expect("the binary runs");
    let mut lines = std::io::BufReader::new(child.stdout.take().expect("stdout is piped")).lines();
    assert_eq!(lines.next().unwrap().expect("a line arrived"), "1");
    assert_eq!(lines.next().unwrap().expect("a line arrived"), "2");
    assert_eq!(lines.next().unwrap().expect("a line arrived"), "Fizz");
    // A million counts remain, and the pipe holds only what it holds: the
    // run cannot have ended while its lines wait unread.
    assert!(
        child
            .try_wait()
            .expect("the exit status is readable")
            .is_none(),
        "the run is still streaming, not dumping at the end"
    );
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_file(&path);
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
    // `utility/format` is not linked: this binary carries only what its
    // graphs need, and the compiler refuses what no linked plugin declares.
    let path = std::env::temp_dir().join("nodetool-fizzbuzz-uncompilable.yml");
    std::fs::write(
        &path,
        "schema_version: 1\nnodes:\n  - uuid: 00000000-0000-0000-0000-200000000001\n    type_ref: utility/format\nedges: []\n",
    )
    .expect("the file is written");

    let output = binary().arg(&path).output().expect("the binary runs");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("compile error"), "{stderr}");
    assert!(stderr.contains("utility/format"), "{stderr}");
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
        stderr.contains("00000000-0000-0000-0000-300000000002"),
        "{stderr}"
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
