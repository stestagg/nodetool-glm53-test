//! Loads the sample graph file shipped with this example and prints what it
//! loaded — each node's uuid, type reference, label, parameters, and
//! metadata, then the edges — dumps it back to YAML to show the round trip,
//! and attempts a deliberately malformed file to show the load error.

use plugin_shapes as _;
use plugin_text as _;

use nodetool::graph::{self, GraphDefinition, NodeInstance};

const SAMPLE: &str = include_str!("../graphs/sample.yml");
const MALFORMED: &str = include_str!("../graphs/malformed.yml");

fn main() {
    println!("loading graphs/sample.yml");
    match graph::load(SAMPLE) {
        Ok(definition) => {
            print_definition(&definition);
            show_round_trip(&definition);
        }
        Err(error) => println!("error: {error}"),
    }

    println!();
    println!("loading graphs/malformed.yml");
    match graph::load(MALFORMED) {
        Ok(definition) => print_definition(&definition),
        Err(error) => println!("error: {error}"),
    }
}

fn print_definition(definition: &GraphDefinition) {
    if let Some(name) = &definition.name {
        println!("name: {name}");
    }
    for node in &definition.nodes {
        print_node(node);
    }
    for edge in &definition.edges {
        println!(
            "edge {} {} -> {} {}",
            edge.from, edge.from_port, edge.to, edge.to_port
        );
    }
}

fn print_node(node: &NodeInstance) {
    match &node.label {
        Some(label) => println!("node {} [{}] label: {label}", node.uuid, node.type_ref),
        None => println!("node {} [{}]", node.uuid, node.type_ref),
    }
    if !node.parameters.is_empty() {
        println!("  parameters:");
        let yaml = serde_yaml::to_string(&node.parameters).expect("parameters always serialise");
        for line in yaml.lines() {
            println!("    {line}");
        }
    }
    if !node.metadata.is_empty() {
        println!("  metadata:");
        let yaml = serde_yaml::to_string(&node.metadata).expect("metadata always serialises");
        for line in yaml.lines() {
            println!("    {line}");
        }
    }
}

fn show_round_trip(definition: &GraphDefinition) {
    let dumped = graph::dump(definition);
    println!();
    println!("dumped back to YAML:");
    println!("{dumped}");
    match graph::load(&dumped) {
        Ok(round_tripped) if round_tripped == *definition => {
            println!("the dump loads back identical");
        }
        Ok(_) => println!("the dump loads back different"),
        Err(error) => println!("the dump fails to load: {error}"),
    }
}
