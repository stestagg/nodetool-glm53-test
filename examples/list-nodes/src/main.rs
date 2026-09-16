//! Lists every node type the linked plugins contribute to the registry,
//! grouped by plugin, then sub-group, then label — and every registered data
//! type with its metadata and declared conversions.

use plugin_shapes as _;
use plugin_text as _;

fn main() {
    let mut node_types: Vec<_> = nodetool::registry::node_types().collect();
    node_types.sort_unstable_by_key(|node| (node.plugin, node.sub_group, node.label));
    if node_types.is_empty() {
        println!("no node types registered");
    } else {
        for node_type in node_types {
            println!("{node_type}");
        }
    }

    println!();
    println!("data types:");
    let mut data_types: Vec<_> = nodetool::registry::data_types().collect();
    data_types.sort_unstable_by_key(|data_type| data_type.name);
    for data_type in data_types {
        println!("  {data_type}");
        for conversion in data_type.conversions {
            let target = nodetool::registry::data_type_by_id(conversion.target)
                .expect("conversion targets are validated when the registry is read");
            println!("    converts to {}", target.name);
        }
    }
}
