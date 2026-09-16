//! The same listing with no plugin linked: core contributes no node types,
//! and the base scalar set ships with it.

fn main() {
    println!("node types:");
    let mut found = false;
    for node_type in nodetool::registry::node_types() {
        found = true;
        println!("{node_type}");
    }
    if !found {
        println!("no node types registered");
    }

    println!();
    println!("data types:");
    let mut data_types: Vec<_> = nodetool::registry::data_types().collect();
    data_types.sort_unstable_by_key(|data_type| data_type.name.to_lowercase());
    for data_type in data_types {
        println!("  {data_type}");
        for conversion in data_type.conversions {
            let target = nodetool::registry::data_type_by_id(conversion.target)
                .expect("conversion targets are validated when the registry is read");
            println!("    converts to {}", target.name);
        }
    }
}
