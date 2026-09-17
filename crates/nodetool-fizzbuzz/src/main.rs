//! Lists what the fizzbuzz plugin contributes to the registry: one
//! Counter, the six Condition operations under their sub-group, and one
//! Output — each with its ports and the concrete set of types each port
//! spans, the numeric ports naming the numeric family's members. Running
//! this binary is the proof the crate links in like any plugin.

use nodetool_fizzbuzz as _;

fn main() {
    println!("the fizzbuzz plugin contributes:");
    let mut node_types: Vec<_> = nodetool::registry::node_types()
        .filter(|node_type| node_type.plugin == "fizzbuzz")
        .collect();
    node_types.sort_unstable_by_key(|node_type| (node_type.sub_group, node_type.label));
    for node_type in node_types {
        println!("{node_type}");
    }
}
