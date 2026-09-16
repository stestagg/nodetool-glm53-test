//! The same listing with no plugin linked: core alone contributes no node
//! types.

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
