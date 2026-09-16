//! The demo binary's terminal behavior: the pipeline sample completes and
//! prints its values as they arrive; the failing sample exits non-zero with
//! the error naming the node; a load error and compile errors each end the
//! path printed and non-zero.

use std::process::Command;

/// Runs the demo binary on one sample, returning stdout, stderr, and the
/// exit code.
fn run(sample: &str) -> (String, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_run-graph"))
        .arg(sample)
        .output()
        .expect("the demo binary runs");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code(),
    )
}

#[test]
fn the_pipeline_sample_prints_its_values_as_they_arrive_and_completes() {
    let (out, err, code) = run("pipeline");
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    for part in ["alpha", "beta", "gamma", "one"] {
        assert!(
            out.contains(&format!("emitted {part}\n")),
            "the value printed as it arrived: {out}"
        );
    }
    assert!(
        out.contains("the run completed\n"),
        "the run's completion printed last: {out}"
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
    assert!(
        err.contains("no sample named"),
        "the refusal named: {err}"
    );
}
