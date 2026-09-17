//! The demo binary's terminal behavior: the pipeline sample completes and
//! prints its values as they arrive; the failing sample's values stop
//! arriving — the guard's rejection ends the run before the withheld lines
//! can print — and the error naming the node is the last word before a
//! non-zero exit; a load error and compile errors each end the path
//! printed and non-zero.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Runs the demo binary on one sample, returning stdout, stderr, and the
/// exit code. The one-second bound keeps a regression that never ends a
/// loud failure, the way the engine tests bound their runs.
fn run(sample: &str) -> (String, String, Option<i32>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_run-graph"))
        .arg(sample)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the demo binary runs");
    let deadline = Instant::now() + Duration::from_secs(1);
    let status = loop {
        match child
            .try_wait()
            .expect("the demo binary's exit status is readable")
        {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the demo binary ran past one second; a run must end: {sample}");
            }
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .stdout
        .take()
        .expect("stdout is piped")
        .read_to_string(&mut stdout)
        .expect("stdout is readable");
    child
        .stderr
        .take()
        .expect("stderr is piped")
        .read_to_string(&mut stderr)
        .expect("stderr is readable");
    (stdout, stderr, status.code())
}

#[test]
fn the_pipeline_sample_prints_its_values_as_they_arrive_and_completes() {
    let (out, err, code) = run("pipeline");
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert_eq!(err, "", "nothing on the error stream: {err}");
    assert_eq!(
        out,
        "emitted alpha\nemitted beta\nemitted gamma\nemitted one\nemitted one\nemitted one\nthe run completed\n",
        "each value printed as it arrived, the completion last"
    );
}

#[test]
fn the_failing_sample_exits_non_zero_with_the_error_naming_the_node() {
    let (out, err, code) = run("failing");
    assert_ne!(code, Some(0), "the failed run exits non-zero: {out}\n{err}");
    assert!(
        out.contains("emitted alpha\n"),
        "values flowed before the failure: {out}"
    );
    assert!(
        !out.contains("emitted one\n"),
        "the guard rejected the second line's first value, so none of its values passed: {out}"
    );
    assert!(
        !out.contains("emitted delta\n"),
        "the third line was still pending when the run ended and never arrived: {out}"
    );
    assert!(err.contains("the run failed"), "the failure is told: {err}");
    assert!(err.contains("the guard"), "the node's label named: {err}");
    assert!(
        err.contains("00000000-0000-0000-0000-200000000003"),
        "the node's uuid named: {err}"
    );
    assert!(
        err.contains("the value \"one\" is rejected here"),
        "what went wrong told: {err}"
    );
}

#[test]
fn a_load_error_ends_the_path_printed_and_non_zero() {
    let (out, err, code) = run("broken");
    assert_ne!(
        code,
        Some(0),
        "a file that fails to load exits non-zero: {out}\n{err}"
    );
    assert!(err.contains("load error"), "the load error printed: {err}");
    assert!(
        err.contains("unknown field `surprise`"),
        "the error names what: {err}"
    );
    assert!(
        err.contains("document.surprise"),
        "the error names where: {err}"
    );
}

#[test]
fn compile_errors_end_the_path_printed_and_non_zero() {
    let (out, err, code) = run("uncompilable");
    assert_ne!(
        code,
        Some(0),
        "a file that fails to compile exits non-zero: {out}\n{err}"
    );
    assert!(
        err.contains("compile error"),
        "the compile error printed: {err}"
    );
    assert!(
        err.contains("text/nothing"),
        "the error names the type reference: {err}"
    );
    assert!(
        err.contains("00000000-0000-0000-0000-300000000001"),
        "the error names the node: {err}"
    );
}

#[test]
fn an_unknown_sample_is_refused() {
    let (_, err, code) = run("no-such-sample");
    assert_eq!(code, Some(2), "the unknown sample refused: {err}");
    assert!(err.contains("no sample named"), "the refusal named: {err}");
}
