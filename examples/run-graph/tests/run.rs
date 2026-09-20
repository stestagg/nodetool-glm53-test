//! The demo binary's terminal behavior: the pipeline sample completes and
//! prints its values as they arrive; the failing sample's values stop
//! arriving — the guard's rejection ends the run before the withheld lines
//! can print — and the error naming the node is the last word before a
//! non-zero exit; a load error and compile errors each end the path
//! printed and non-zero. The `observe` flag asks for the engine's event
//! timeline beside the values: the same run, told line by line.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Runs the demo binary with these arguments, returning stdout, stderr,
/// and the exit code. The one-second bound keeps a regression that never
/// ends a loud failure, the way the engine tests bound their runs.
fn run(args: &[&str]) -> (String, String, Option<i32>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_run-graph"))
        .args(args)
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
                panic!("the demo binary ran past one second; a run must end: {args:?}");
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
    let (out, err, code) = run(&["pipeline"]);
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert_eq!(err, "", "nothing on the error stream: {err}");
    assert_eq!(
        out,
        "consumed: alpha\nconsumed: beta\nconsumed: gamma\nconsumed: one\nconsumed: one\nconsumed: one\nthe run completed\n",
        "each value printed as it arrived, the completion last"
    );
}

#[test]
fn the_observed_pipeline_tells_the_timeline_and_the_same_values() {
    let (timeline, err, code) = run(&["pipeline", "observe"]);
    assert_eq!(code, Some(0), "stdout: {timeline}\nstderr: {err}");
    assert_eq!(err, "", "nothing on the error stream: {err}");

    let lines = timeline.lines().collect::<Vec<_>>();
    assert_eq!(
        lines.first(),
        Some(&"run started"),
        "the timeline opens with the run's start: {timeline}"
    );
    assert_eq!(
        lines.last(),
        Some(&"the run completed"),
        "the outcome follows the timeline, which the run finished closed: {timeline}"
    );
    for told in [
        "lines started",
        "lines emitted out: \"alpha, beta, gamma\"",
        "splitter started",
        "splitter emitted parts: \"alpha\"",
        "shout one emitted text: \"ALPHA\"",
        "shout two emitted text: \"GAMMA\"",
        "lines completed",
        "splitter completed",
        "shout one completed",
        "shout two completed",
        "run completed",
    ] {
        assert!(
            lines.contains(&told),
            "the timeline tells `{told}`: {timeline}"
        );
    }
    for consumed in [
        "consumed: alpha",
        "consumed: beta",
        "consumed: gamma",
        "consumed: one",
    ] {
        assert!(
            lines.contains(&consumed),
            "the values are consumed as before: {timeline}"
        );
    }
}

#[test]
fn the_failing_sample_exits_non_zero_with_the_error_naming_the_node() {
    let (out, err, code) = run(&["failing"]);
    assert_ne!(code, Some(0), "the failed run exits non-zero: {out}\n{err}");
    assert!(
        out.contains("consumed: alpha\n"),
        "values flowed before the failure: {out}"
    );
    assert!(
        !out.contains("consumed: one\n"),
        "the guard rejected the second line's first value, so none of its values passed: {out}"
    );
    assert!(
        !out.contains("consumed: delta\n"),
        "the third line was still pending when the run ended and never arrived: {out}"
    );
    assert!(
        err.contains("the run ended in failure"),
        "the failure is told: {err}"
    );
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
fn the_observed_failing_sample_tells_the_failure_in_the_timeline_and_fails_alike() {
    let (timeline, err, code) = run(&["failing", "observe"]);
    assert_ne!(
        code,
        Some(0),
        "the observer changes nothing about the run's end: {timeline}\n{err}"
    );
    assert!(
        err.contains("the run ended in failure: node the guard"),
        "as ever: the run's own report on the error stream: {err}"
    );
    let lines = timeline.lines().collect::<Vec<_>>();
    for told in [
        "run started",
        "the guard started",
        "the guard failed: the value \"one\" is rejected here",
        "run failed: node the guard (00000000-0000-0000-0000-200000000003): the value \"one\" is rejected here",
    ] {
        assert!(
            lines.contains(&told),
            "the timeline tells `{told}`: {timeline}"
        );
    }
    assert_eq!(
        lines.last(),
        Some(&"run failed: node the guard (00000000-0000-0000-0000-200000000003): the value \"one\" is rejected here"),
        "the failed run-finished closes the timeline: {timeline}"
    );
}

#[test]
fn a_load_error_ends_the_path_printed_and_non_zero() {
    let (out, err, code) = run(&["broken"]);
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
    let (out, err, code) = run(&["uncompilable"]);
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
fn the_if_true_sample_prints_the_then_branchs_formatted_strings_and_completes() {
    let (out, err, code) = run(&["if-true"]);
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    // The unselected Format carries no template — its `value` arrives on a
    // branch never steered to — so the hang gate warns and the run still
    // completes: the warning advises, it changes no outcome.
    assert_eq!(
        err,
        "compile warning: node else format (00000000-0000-0000-0000-400000000004): input `template` is neither connected nor parameterised — the node will never fire, hanging the run until it is stopped\n",
        "the unselected branch's starving template warned: {err}"
    );
    assert_eq!(
        out,
        "consumed: words: alpha, beta, gamma\nconsumed: words: alpha, beta, gamma\nconsumed: words: one, one, one\nthe run completed\n",
        "every routed string reached the then branch's Format, the template substituting at its first placeholder"
    );
}

#[test]
fn the_if_false_sample_prints_the_else_branchs_plain_strings_and_completes() {
    let (out, err, code) = run(&["if-false"]);
    assert_eq!(code, Some(0), "stdout: {out}\nstderr: {err}");
    assert_eq!(
        err,
        "compile warning: node then format (00000000-0000-0000-0000-500000000003): input `template` is neither connected nor parameterised — the node will never fire, hanging the run until it is stopped\n",
        "the unselected branch's starving template warned: {err}"
    );
    assert_eq!(
        out,
        "consumed: alpha, beta, gamma\nconsumed: alpha, beta, gamma\nconsumed: one, one, one\nthe run completed\n",
        "the same graph steered to the else branch, the empty template leaving the plain string form"
    );
}

#[test]
fn an_unknown_sample_is_refused() {
    let (_, err, code) = run(&["no-such-sample"]);
    assert_eq!(code, Some(2), "the unknown sample refused: {err}");
    assert!(err.contains("no sample named"), "the refusal named: {err}");
}
