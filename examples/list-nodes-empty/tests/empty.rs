#[test]
fn core_alone_registers_no_node_types() {
    assert_eq!(nodetool::registry::node_types().count(), 0);
}
