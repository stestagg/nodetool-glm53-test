//! The headless path from a terminal: load a graph file, compile it, run
//! it, and consume a node's output as one more downstream of its fan-out —
//! each value prints as it arrives, then the run's outcome. A run ends
//! complete, or fail-fast with one error naming the node; a file that fails
//! to load or compile ends the path the same way, printed, with a non-zero
//! exit.
//!
//! One of the built-in sample graphs names the run: `pipeline` (the
//! default), `failing`, `broken`, or `uncompilable`.

use plugin_shapes as _;
use plugin_text as _;

use nodetool::compile;
use nodetool::engine::Run;
use nodetool::graph;
use nodetool::registry::Registry;
use nodetool::{uuid, Uuid};

/// The splitter, whose `parts` output the demo consumes — the same output
/// the graph's own downstream nodes take.
const SPLITTER: Uuid = uuid!("00000000-0000-0000-0000-100000000002");

/// The built-in sample graphs, by the name that selects them.
const SAMPLES: &[(&str, &str)] = &[
    ("pipeline", include_str!("../graphs/pipeline.yml")),
    ("failing", include_str!("../graphs/failure.yml")),
    ("broken", include_str!("../graphs/broken.yml")),
    ("uncompilable", include_str!("../graphs/uncompilable.yml")),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    let asked = std::env::args().nth(1).unwrap_or_else(|| "pipeline".to_owned());
    let Some((_, text)) = SAMPLES.iter().find(|(name, _)| *name == asked) else {
        let names = SAMPLES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ");
        eprintln!("no sample named {asked:?}; samples: {names}");
        return std::process::ExitCode::from(2);
    };

    let definition = match graph::load(text) {
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
    run.consume(SPLITTER, "parts", |mut parts| async move {
        while let Some(value) = parts.recv().await {
            println!(
                "emitted {}",
                value.get::<String>().expect("`parts` is declared String")
            );
        }
        Ok(())
    });
    match run.start().await {
        Ok(()) => {
            println!("the run completed");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("the run failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
