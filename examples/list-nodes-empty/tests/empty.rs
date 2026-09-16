#[test]
fn core_alone_registers_no_node_types() {
    assert_eq!(nodetool::registry::node_types().count(), 0);
}

#[test]
fn core_ships_the_base_scalars_without_any_plugin() {
    let names: Vec<&str> = nodetool::registry::data_types().map(|t| t.name).collect();
    assert_eq!(names.len(), 12);
    for name in [
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "String",
    ] {
        assert!(names.contains(&name), "missing {name} in {names:?}");
    }
}
