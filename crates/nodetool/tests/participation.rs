//! Crate participation: a plugin crate linked as a dependency but whose items
//! are never referenced by the test code must still contribute its declared
//! node types. The only mention of each plugin here is its anchor import.

use test_plugin_alpha as _;
use test_plugin_beta as _;

#[test]
fn linked_plugins_contribute_without_being_referenced() {
    let refs: Vec<&str> = nodetool::registry::node_types()
        .map(|node_type| node_type.type_ref)
        .collect();
    assert_eq!(refs.len(), 4);
    for expected in ["alpha/add", "alpha/concat", "beta/identity", "beta/tick"] {
        assert!(refs.contains(&expected), "missing {expected} in {refs:?}");
    }
}
