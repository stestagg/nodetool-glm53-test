//! Lists every node type the linked plugins contribute to the registry,
//! grouped by plugin, then sub-group, then label.

use plugin_shapes as _;
use plugin_text as _;

fn main() {
    let mut node_types: Vec<_> = nodetool::registry::node_types().collect();
    node_types.sort_unstable_by_key(|node| (node.plugin, node.sub_group, node.label));
    if node_types.is_empty() {
        println!("no node types registered");
        return;
    }
    for node_type in node_types {
        println!("{node_type}");
    }
}
