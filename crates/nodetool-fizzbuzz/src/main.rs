//! The headless runner from a terminal: `nodetool-fizzbuzz <graph-file>`
//! loads the file (the one graph file format), compiles it against the
//! linked plugin's node types, and runs it. Every output the graph leaves
//! unconnected is the terminal's: each value arriving on one prints as it
//! arrives — one line, in its plain string form — nothing held back to the
//! end. Any graph the linked nodes can express runs the same way; the
//! shipped proof is `graphs/fizzbuzz.yml`.
//!
//! Values go to stdout. A file that cannot be read or loaded, a graph that
//! fails to compile, and a run that ends in an error go to stderr, naming
//! what and where, with a non-zero exit.

use std::process::ExitCode;

use nodetool::compile;
use nodetool::engine::Run;
use nodetool::graph;
use nodetool::registry::{self, Registry};
use nodetool::Value;
use nodetool_fizzbuzz as _;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: nodetool-fizzbuzz <graph-file>");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
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
    let compiled = match compile::compile(&definition, &Registry::collect()) {
        Ok(compiled) => compiled,
        Err(errors) => {
            for error in errors {
                eprintln!("compile error: {error}");
            }
            return ExitCode::FAILURE;
        }
    };

    // The terminal completes every output the graph leaves unconnected:
    // one consumer per such output, joined to its fan-out like any other
    // downstream, printing each value as it arrives.
    let mut run = Run::new(&compiled);
    for (uuid, node) in &compiled.nodes {
        for port in node.node_type.outputs {
            let fed = compiled
                .connections
                .iter()
                .any(|connection| connection.from == *uuid && connection.from_port == port.name);
            if fed {
                continue;
            }
            run.consume(*uuid, port.name, |mut values| async move {
                while let Some(value) = values.recv().await {
                    println!("{}", plain(&value));
                }
                Ok(())
            });
        }
    }

    match run.start().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("the run ended in failure: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The plain string form of a value — the Format node's without-template
/// behaviour: the payload as its Rust `Display` renders it, a string as
/// itself. A value outside the base scalars is named rather than rendered;
/// rendering a custom type is its plugin's business.
fn plain(value: &Value) -> String {
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
