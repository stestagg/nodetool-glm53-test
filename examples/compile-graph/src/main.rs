//! Loads each sample graph shipped with this example and compiles it
//! against the linked plugins' registry: the good sample prints as the
//! compiled graph — nodes with their resolved parameter types, connections
//! with resolved types and conversions marked — and the deliberately broken
//! samples print the compile errors that stand between them and a runnable
//! graph.

use plugin_shapes as _;
use plugin_text as _;

use nodetool::compile::{self, CompiledGraph, CompiledParameter};
use nodetool::graph;
use nodetool::registry::Registry;

const OK: &str = include_str!("../graphs/ok.yml");
const UNKNOWN_TYPE: &str = include_str!("../graphs/unknown-type.yml");
const CYCLE: &str = include_str!("../graphs/cycle.yml");
const UNRESOLVABLE: &str = include_str!("../graphs/unresolvable.yml");
const BAD_LITERAL: &str = include_str!("../graphs/bad-literal.yml");

fn main() {
    let registry = Registry::collect();
    for (file, text) in [
        ("graphs/ok.yml", OK),
        ("graphs/unknown-type.yml", UNKNOWN_TYPE),
        ("graphs/cycle.yml", CYCLE),
        ("graphs/unresolvable.yml", UNRESOLVABLE),
        ("graphs/bad-literal.yml", BAD_LITERAL),
    ] {
        println!("{file}:");
        match graph::load(text) {
            Ok(definition) => {
                let result = compile::compile(&definition, &registry);
                for warning in &result.warnings {
                    println!("  compile warning: {}", warning.message);
                }
                match result.graph {
                    Some(compiled) => print_compiled(&compiled),
                    None => {
                        for error in &result.errors {
                            println!("  compile error: {}", error.message);
                        }
                    }
                }
            }
            Err(error) => println!("  load error: {error}"),
        }
        println!();
    }
}

/// Present a compiled parameter: the demo knows the base scalars it links,
/// so it reads each value as the concrete type its resolved name declares —
/// presentation lives here, with the listing, not in core, where the
/// payload is erased. Floats render through `Debug` so their kind shows: an
/// integral float reads as `3.0`, never the integer `3`. Anything else
/// stays opaque, as it does everywhere outside the declaring plugin.
fn present(parameter: &CompiledParameter) -> String {
    let value = &parameter.value;
    let shown = match parameter.resolved_type.name {
        "String" => value.get::<String>().map(|v| format!("{v:?}")),
        "bool" => value.get::<bool>().map(|&v| v.to_string()),
        "i8" => value.get::<i8>().map(|&v| v.to_string()),
        "i16" => value.get::<i16>().map(|&v| v.to_string()),
        "i32" => value.get::<i32>().map(|&v| v.to_string()),
        "i64" => value.get::<i64>().map(|&v| v.to_string()),
        "u8" => value.get::<u8>().map(|&v| v.to_string()),
        "u16" => value.get::<u16>().map(|&v| v.to_string()),
        "u32" => value.get::<u32>().map(|&v| v.to_string()),
        "u64" => value.get::<u64>().map(|&v| v.to_string()),
        "f32" => value.get::<f32>().map(|v| format!("{v:?}")),
        "f64" => value.get::<f64>().map(|v| format!("{v:?}")),
        _ => None,
    };
    shown.unwrap_or_else(|| format!("<{}>", parameter.resolved_type.name))
}

fn print_compiled(compiled: &CompiledGraph) {
    if let Some(name) = &compiled.name {
        println!("  name: {name}");
    }
    for (uuid, node) in &compiled.nodes {
        println!("  node {uuid} [{}]", node.node_type.type_ref);
        for (port, parameter) in &node.parameters {
            println!(
                "    {port} = {} [{}]",
                present(parameter),
                parameter.resolved_type.name
            );
        }
    }
    for connection in &compiled.connections {
        let from_type = compiled.nodes[&connection.from].node_type.type_ref;
        let to_type = compiled.nodes[&connection.to].node_type.type_ref;
        println!(
            "  connection {} [{}] `{}` → {} [{}] `{}` : {}{}",
            connection.from,
            from_type,
            connection.from_port,
            connection.to,
            to_type,
            connection.to_port,
            connection.resolved_type.name,
            if connection.conversion.is_some() {
                " (converted)"
            } else {
                ""
            },
        );
    }
}
