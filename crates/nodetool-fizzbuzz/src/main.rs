//! The fizzbuzz binary, both doors of one graph file.
//!
//! Headless — the default — `nodetool-fizzbuzz <graph-file>` loads the
//! file (the one graph file format), compiles it against the linked
//! plugin's node types, and runs it. Every output the graph leaves
//! unconnected is the terminal's: each value arriving on one prints as it
//! arrives — one line, in its plain string form — nothing held back to the
//! end. Any graph the linked nodes can express runs the same way; the
//! shipped proof is `graphs/fizzbuzz.yml`.
//!
//! `nodetool-fizzbuzz --ui [graph-file]` hosts the editor instead: the
//! server starts on its loopback default, the file — when one is given —
//! is loaded into the held definition through the same loader, so a file
//! the headless run takes is the file the editor opens, and the served
//! address is printed; opening it in a browser shows the editor over this
//! binary's own linked nodes. The terminal keeps its seat: the binary
//! hands the server the printer for the outputs a run leaves unconnected,
//! so a run started from the browser prints here exactly as a headless
//! run does. Quitting is guarded at the process edge: Ctrl-C over unsaved
//! changes warns and stands down, and the next Ctrl-C quits.
//!
//! Values go to stdout. A file that cannot be read or loaded, a graph
//! that fails to compile, and a run that ends in an error go to stderr,
//! naming what and where, with a non-zero exit — in UI mode only the
//! launch's load faults do, a session past its launch reporting through
//! the editor's own surfaces.

use std::io;
use std::process::ExitCode;
use std::sync::Arc;

use tokio::signal;
use tokio::sync::watch;

use nodetool::behaviour::Receiver;
use nodetool::compile;
use nodetool::engine::{self, Run};
use nodetool::graph;
use nodetool::registry::{self, Registry};
use nodetool::server::{read_definition, Editor, UnconnectedStream, DEFAULT_ADDRESS};
use nodetool::Value;
use nodetool_fizzbuzz as _;

const USAGE: &str =
    "usage: nodetool-fizzbuzz <graph-file>\n       nodetool-fizzbuzz --ui [graph-file]";

/// What the invocation asked for: the headless run of a file, or the
/// editor over a file — or over nothing, an empty canvas.
#[derive(Debug, PartialEq)]
enum Mode {
    Headless(String),
    Ui(Option<String>),
}

fn mode(args: &mut impl Iterator<Item = String>) -> Result<Mode, &'static str> {
    match args.next().as_deref() {
        Some("--ui") => match (args.next(), args.next()) {
            (Some(file), None) => Ok(Mode::Ui(Some(file))),
            (None, None) => Ok(Mode::Ui(None)),
            _ => Err(USAGE),
        },
        Some(path) => match args.next() {
            None => Ok(Mode::Headless(path.to_owned())),
            Some(_) => Err(USAGE),
        },
        None => Err(USAGE),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match mode(&mut std::env::args().skip(1)) {
        Ok(Mode::Headless(path)) => headless(&path).await,
        Ok(Mode::Ui(file)) => ui(file).await,
        Err(usage) => {
            eprintln!("{usage}");
            ExitCode::from(2)
        }
    }
}

async fn headless(path: &str) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let definition = match graph::load(&text) {
        Ok(definition) => definition,
        Err(error) => {
            eprintln!("load error in {path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let result = compile::compile(&definition, &Registry::collect());
    for warning in &result.warnings {
        eprintln!("compile warning: {}", warning.message);
    }
    let compiled = match result.graph {
        Some(compiled) => compiled,
        None => {
            for error in &result.errors {
                eprintln!("compile error: {}", error.message);
            }
            return ExitCode::FAILURE;
        }
    };

    // The terminal completes every output the graph leaves unconnected:
    // one consumer per such output, joined to its fan-out like any other
    // downstream, printing each value as it arrives. The run ends with the
    // terminal: a write that fails — the reader went away, `head` or a
    // pager done — stops the run quietly, not as a failure, and no panic
    // ever reaches the terminal.
    let mut run = Run::new(&compiled);
    let (stop, stopped) = watch::channel(false);
    run.stop_on(stopped);
    for (uuid, port) in engine::unconnected_outputs(&compiled) {
        let stop = stop.clone();
        run.consume(uuid, port, move |values| {
            print_values(io::stdout(), values, Some(stop))
        });
    }

    match run.start().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("the run ended in failure: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn ui(file: Option<String>) -> ExitCode {
    let definition = match file.as_deref().map(read_definition) {
        Some(Ok(definition)) => definition,
        Some(Err(error)) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
        None => graph::GraphDefinition::empty(),
    };
    let listener = match tokio::net::TcpListener::bind(DEFAULT_ADDRESS).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "cannot bind {DEFAULT_ADDRESS}: {error}; another editor is probably already running"
            );
            return ExitCode::FAILURE;
        }
    };
    let editor =
        Arc::new(Editor::new(definition, file).consume_unconnected(Arc::new(print_stream)));
    let address = listener.local_addr().expect("the listener is bound");
    let mut serving = tokio::spawn(Arc::clone(&editor).serve(listener));
    println!("fizzbuzz editor on http://{address}");
    tokio::select! {
        served = &mut serving => match served.expect("the editor server task failed") {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        _ = signal::ctrl_c() => {
            quit_guard(&editor).await;
            ExitCode::SUCCESS
        }
    }
}

/// The process edge of the editor's unsaved-changes confirm: a Ctrl-C
/// over unsaved changes warns and stands down — the session carries on,
/// saving from the browser still lands — and the next Ctrl-C is the
/// user's word they know, quitting at once. Nothing unsaved, the first
/// Ctrl-C quits promptly.
async fn quit_guard(editor: &Editor) {
    if editor.unsaved_changes() {
        eprintln!("the graph has unsaved changes — press Ctrl-C again to quit without saving");
        signal::ctrl_c()
            .await
            .expect("the interrupt listener is installed");
    }
}

/// The printer the UI mode supplies to the server: the headless rule
/// verbatim. A write that fails — the reader went away — ends the
/// printing; the run itself is the editor's, its outcome the browser's.
fn print_stream(values: Receiver<Value>) -> UnconnectedStream {
    Box::pin(print_values(io::stdout(), values, None))
}

/// Story 10's printing rule, one body for both doors: every value the
/// output delivers, one line per value, in arrival order, in its plain
/// string form, as it arrives. A write that fails — the reader went away —
/// ends the printing; headless passes the run's stop channel through
/// `stop`, the run ending with the terminal, while UI mode passes none,
/// the run itself the editor's, its outcome the browser's.
async fn print_values<W: io::Write>(
    mut writer: W,
    mut values: Receiver<Value>,
    stop: Option<watch::Sender<bool>>,
) -> Result<(), nodetool::behaviour::Error> {
    while let Some(value) = values.recv().await {
        if writeln!(writer, "{}", plain(&value)).is_err() {
            if let Some(stop) = stop {
                let _ = stop.send(true);
            }
            break;
        }
    }
    Ok(())
}

/// The plain string form of a value — the Format node's without-template
/// behaviour: the payload as its Rust `Display` renders it, a string as
/// itself. A value outside the base scalars is named rather than rendered;
/// rendering a custom type is its plugin's business.
fn plain(value: &Value) -> String {
    // The scalar list is the twin of `string_form` in crates/nodetool-utility:
    // the same twelve base scalars, kept in step by hand — linking that
    // crate here would register its node types, which this binary does not.
    macro_rules! scalars {
        ($($ty:ty),* $(,)?) => {$(
            if let Some(form) = value.get::<$ty>().map(ToString::to_string) {
                return form;
            }
        )*};
    }
    scalars!(String, bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
    match registry::data_type_by_id(value.type_id()) {
        Some(data_type) => format!("a {} value", data_type.name),
        None => format!("a value of id {}", value.type_id()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode_of(args: &[&str]) -> Result<Mode, &'static str> {
        mode(&mut args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn the_invocations_parse() {
        assert_eq!(mode_of(&["g.yml"]), Ok(Mode::Headless("g.yml".into())));
        assert_eq!(mode_of(&["--ui"]), Ok(Mode::Ui(None)));
        assert_eq!(
            mode_of(&["--ui", "g.yml"]),
            Ok(Mode::Ui(Some("g.yml".into())))
        );
    }

    #[test]
    fn a_bare_invocation_and_a_doubled_file_argument_are_usage_errors() {
        assert_eq!(mode_of(&[]), Err(USAGE));
        assert_eq!(mode_of(&["--ui", "a.yml", "b.yml"]), Err(USAGE));
    }

    #[test]
    fn anything_after_the_file_is_a_usage_error_not_a_silent_mode_change() {
        assert_eq!(mode_of(&["g.yml", "--ui"]), Err(USAGE));
        assert_eq!(mode_of(&["g.yml", "extra"]), Err(USAGE));
    }

    /// A writer that refuses every write, as a closed pipe does.
    struct Refusing;
    impl io::Write for Refusing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("the reader went away"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn a_failed_write_ends_the_ui_printing_quietly_not_as_a_failure() {
        // The sender stays alive: a printer that kept reading past the
        // failed write instead of ending would hang, and the bound names it.
        let (sender, values) = tokio::sync::mpsc::channel(4);
        sender
            .send(Value::new(nodetool::scalars::I32, 1i32))
            .await
            .expect("the value queues");
        let printed = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            print_values(Refusing, values, None),
        )
        .await
        .expect("the printing ends; a failed write does not hang it");
        assert!(
            matches!(printed, Ok(())),
            "the stream's end is not an error: {printed:?}"
        );
    }
}
