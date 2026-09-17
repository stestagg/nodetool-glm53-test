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

/// A built-in sample graph.
struct Sample {
    name: &'static str,
    text: &'static str,
    /// The node whose output the demo consumes — the same output the
    /// graph's own downstream nodes take. A sample that cannot reach a run
    /// carries none.
    consumed: Option<Uuid>,
}

/// The built-in sample graphs, by the name that selects them.
const SAMPLES: &[Sample] = &[
    Sample {
        name: "pipeline",
        text: include_str!("../graphs/pipeline.yml"),
        consumed: Some(uuid!("00000000-0000-0000-0000-100000000002")),
    },
    Sample {
        name: "failing",
        text: include_str!("../graphs/failure.yml"),
        consumed: Some(uuid!("00000000-0000-0000-0000-200000000002")),
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    let asked = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "pipeline".to_owned());
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
    let consumed = sample
        .consumed
        .expect("every sample that reaches a run names the node the demo consumes");
    run.consume(consumed, "parts", |mut parts| async move {
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
