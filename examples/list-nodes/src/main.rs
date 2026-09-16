//! Lists every node type the linked plugins contribute to the registry.

use plugin_shapes as _;
use plugin_text as _;

fn main() {
    let mut found = false;
    for node_type in nodetool::registry::node_types() {
        found = true;
        println!("{node_type}");
    }
    if !found {
        println!("no node types registered");
    }
}
