//! Registry behaviour over the two test plugins.

use nodetool::registry::{node_type, node_types};
use nodetool::NodeType;
use test_plugin_alpha as _;
use test_plugin_beta as _;

fn refs_of(filter: impl Fn(&NodeType) -> bool) -> Vec<&'static str> {
    node_types()
        .filter(|node_type| filter(node_type))
        .map(|node_type| node_type.type_ref)
        .collect()
}

#[test]
fn aggregates_node_types_across_plugins() {
    let refs = refs_of(|_| true);
    assert_eq!(refs.len(), 4);
    for expected in ["alpha/add", "alpha/concat", "beta/identity", "beta/tick"] {
        assert!(refs.contains(&expected), "missing {expected} in {refs:?}");
    }
}

#[test]
fn enumerates_no_utility_node_types_without_the_crate_linked() {
    // A binary linking other plugins but not the utility crate: its listing
    // carries no utility node types — linking a plugin is the only way one
    // arrives.
    assert!(
        refs_of(|node_type| node_type.plugin == "utility").is_empty(),
        "no utility node types without the utility crate linked"
    );
}

#[test]
fn separates_plugins() {
    let mut alpha = refs_of(|node_type| node_type.plugin == "alpha");
    alpha.sort_unstable();
    assert_eq!(alpha, ["alpha/add", "alpha/concat"]);

    let mut beta = refs_of(|node_type| node_type.plugin == "beta");
    beta.sort_unstable();
    assert_eq!(beta, ["beta/identity", "beta/tick"]);
}

#[test]
fn separates_sub_groups_within_a_plugin() {
    let math =
        refs_of(|node_type| node_type.plugin == "alpha" && node_type.sub_group == Some("math"));
    assert_eq!(math, ["alpha/add"]);

    let text =
        refs_of(|node_type| node_type.plugin == "alpha" && node_type.sub_group == Some("text"));
    assert_eq!(text, ["alpha/concat"]);

    let ungrouped = refs_of(|node_type| node_type.sub_group.is_none());
    assert_eq!(ungrouped.len(), 2);
    assert!(ungrouped.contains(&"beta/identity"));
    assert!(ungrouped.contains(&"beta/tick"));
}

#[test]
fn returns_descriptors_exactly_as_declared() {
    let add = node_type("alpha/add").expect("alpha/add is declared by test plugin alpha");
    assert_eq!(add.label, "Add");
    assert_eq!(add.icon, "<svg/>");
    assert_eq!(add.plugin, "alpha");
    assert_eq!(add.sub_group, Some("math"));
    assert_eq!(
        add.inputs,
        [
            nodetool::Port {
                name: "a",
                type_refs: &["i32"],
                family: None
            },
            nodetool::Port {
                name: "b",
                type_refs: &["i32"],
                family: None
            },
        ]
        .as_slice()
    );
    assert_eq!(
        add.outputs,
        [nodetool::Port {
            name: "sum",
            type_refs: &["i32"],
            family: None
        }]
        .as_slice()
    );
}

#[test]
fn ports_carry_one_or_more_type_references() {
    let identity = node_type("beta/identity").expect("declared by test plugin beta");
    assert_eq!(identity.inputs[0].name, "value");
    assert_eq!(identity.inputs[0].type_refs, ["i32", "f64"]);

    let tick = node_type("beta/tick").expect("declared by test plugin beta");
    assert!(tick.inputs.is_empty());
    assert_eq!(tick.outputs[0].type_refs, ["bool"]);
}

#[test]
fn looks_up_by_type_reference() {
    assert!(node_type("beta/tick").is_some());
    assert!(node_type("missing/node").is_none());
}
