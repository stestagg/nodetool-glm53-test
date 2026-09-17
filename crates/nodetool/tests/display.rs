//! The listing a node type renders: what the registry hands to users.

use nodetool::{NodeType, Port};

#[test]
fn display_shows_the_grouping_and_every_port_with_its_type_references() {
    let node = NodeType {
        type_ref: "shapes/circle",
        label: "Circle",
        icon: "<svg/>",
        plugin: "shapes",
        sub_group: Some("2d"),
        inputs: &[Port {
            name: "radius",
            type_refs: &["i32", "f64"],
            family: None,
        }],
        outputs: &[Port {
            name: "shape",
            type_refs: &["shapes/shape"],
            family: None,
        }],
        behaviour: None,
        check_parameters: None,
    };
    let listing = node.to_string();
    assert!(listing.contains("Circle"), "label missing: {listing}");
    assert!(
        listing.contains("shapes/circle"),
        "reference missing: {listing}"
    );
    assert!(
        listing.contains("plugin shapes"),
        "plugin missing: {listing}"
    );
    assert!(
        listing.contains("sub-group 2d"),
        "sub-group missing: {listing}"
    );
    assert!(
        listing.contains("radius: i32, f64"),
        "input missing: {listing}"
    );
    assert!(
        listing.contains("shape: shapes/shape"),
        "output missing: {listing}"
    );
}

#[test]
fn display_omits_an_absent_sub_group() {
    let node = NodeType {
        type_ref: "beta/tick",
        label: "Tick",
        icon: "<svg/>",
        plugin: "beta",
        sub_group: None,
        inputs: &[],
        outputs: &[Port {
            name: "tick",
            type_refs: &["bool"],
            family: None,
        }],
        behaviour: None,
        check_parameters: None,
    };
    assert!(!node.to_string().contains("sub-group"));
}
