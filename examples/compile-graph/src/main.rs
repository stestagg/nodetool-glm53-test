//! Loads each sample graph shipped with this example and compiles it
//! against the linked plugins' registry: the good sample prints as the
//! compiled graph — nodes with their resolved parameter types, connections
//! with resolved types and conversions marked — and the deliberately broken
//! samples print the compile errors that stand between them and a runnable
//! graph.

use plugin_shapes as _;
use plugin_text as _;

use nodetool::compile::{self, CompiledGraph};
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
            Ok(definition) => match compile::compile(&definition, &registry) {
                Ok(compiled) => print_compiled(&compiled),
                Err(errors) => {
                    for error in errors {
                        println!("  compile error: {error}");
                    }
                }
            },
            Err(error) => println!("  load error: {error}"),
        }
        println!();
    }
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
                parameter.value, parameter.resolved_type.name
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
