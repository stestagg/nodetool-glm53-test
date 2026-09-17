//! The headless path from a terminal: load a graph file, compile it, run
//! it, and consume a node's output as one more downstream of its fan-out —
//! each value prints as it arrives, then the run's outcome. A run ends
//! complete, or fail-fast with one error naming the node; a file that fails
//! to load or compile ends the path the same way, printed, with a non-zero
//! exit.
//!
//! The `observe` flag subscribes the core printing observer: one line
//! per event, beside the values. The run itself is the same run either
//! way; the flag changes only who is watching.
//!
//! One of the built-in sample graphs names the run: `pipeline` (the
//! default), `failing`, `broken`, or `uncompilable`; the flag follows the
//! sample (`run-graph failing observe`, or `run-graph observe` for the
//! default sample).

use std::sync::Arc;

use plugin_shapes as _;
use plugin_text as _;

use nodetool::async_trait;
use nodetool::compile;
use nodetool::engine::{Event, Observer, PrintingObserver, Run};
use nodetool::graph;
use nodetool::registry::Registry;
use nodetool::{uuid, Uuid};
use tokio::sync::Notify;

/// A built-in sample graph.
struct Sample {
    name: &'static str,
    text: &'static str,
    /// The node output the demo consumes — one more downstream of its
    /// fan-out, named by node and port. A sample that cannot reach a run
    /// carries none.
    consumed: Option<(Uuid, &'static str)>,
}

/// The built-in sample graphs, by the name that selects them.
const SAMPLES: &[Sample] = &[
    Sample {
        name: "pipeline",
        text: include_str!("../graphs/pipeline.yml"),
        consumed: Some((uuid!("00000000-0000-0000-0000-100000000002"), "parts")),
    },
    Sample {
        name: "failing",
        text: include_str!("../graphs/failure.yml"),
        consumed: Some((uuid!("00000000-0000-0000-0000-200000000003"), "text")),
    },
    Sample {
        name: "broken",
        text: include_str!("../graphs/broken.yml"),
        consumed: None,
    },
    Sample {
        name: "uncompilable",
        text: include_str!("../graphs/uncompilable.yml"),
        consumed: None,
    },
];

/// The demo's listener when the timeline is asked for: the core printing
/// observer composed — the way an embedder composes several listeners
/// behind one observer — with a signal the demo waits on once the
/// timeline's last event, run finished, has been printed.
struct Timeline {
    printed: PrintingObserver,
    printed_all: Arc<Notify>,
}

#[async_trait]
impl Observer for Timeline {
    async fn observe(&self, event: Event) {
        let last = matches!(event, Event::RunFinished { .. });
        self.printed.observe(event).await;
        if last {
            self.printed_all.notify_one();
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    let mut words = std::env::args().skip(1).peekable();
    let (asked, observed) = match words.next() {
        None => ("pipeline".to_owned(), false),
        // `observe` alone: the default sample, watched.
        Some(word) if word == "observe" && words.peek().is_none() => ("pipeline".to_owned(), true),
        Some(name) => {
            let mut observed = false;
            for word in words {
                if word == "observe" && !observed {
                    observed = true;
                } else {
                    eprintln!("no flag named {word:?}; the only flag is `observe`");
                    return std::process::ExitCode::from(2);
                }
            }
            (name, observed)
        }
    };
    let Some(sample) = SAMPLES.iter().find(|sample| sample.name == asked) else {
        let names = SAMPLES
            .iter()
            .map(|sample| sample.name)
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("no sample named {asked:?}; samples: {names}");
        return std::process::ExitCode::from(2);
    };

    let definition = match graph::load(sample.text) {
        Ok(definition) => definition,
        Err(error) => {
            eprintln!("load error: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let compiled = match compile::compile(&definition, &Registry::collect()) {
        Ok(compiled) => compiled,
        Err(errors) => {
            for error in errors {
                eprintln!("compile error: {error}");
            }
            return std::process::ExitCode::FAILURE;
        }
    };

    let mut run = Run::new(&compiled);
    let (consumed, port) = sample
        .consumed
        .expect("every sample that reaches a run names the node output the demo consumes");
    run.consume(consumed, port, |mut parts| async move {
        while let Some(value) = parts.recv().await {
            println!(
                "consumed: {}",
                value
                    .get::<String>()
                    .expect("the consumed port is declared String")
            );
        }
        Ok(())
    });
    let printed_all = if observed {
        let printed_all = Arc::new(Notify::new());
        run.observe(Arc::new(Timeline {
            printed: PrintingObserver::new(),
            printed_all: Arc::clone(&printed_all),
        }));
        Some(printed_all)
    } else {
        None
    };
    let outcome = run.start().await;
    if let Some(printed_all) = printed_all {
        // The run hands events over and moves on: wait for the timeline's
        // own signal that its last event — run finished — was printed,
        // then the outcome follows it on the terminal.
        printed_all.notified().await;
    }
    match outcome {
        Ok(()) => {
            println!("the run completed");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the run ended in failure: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
