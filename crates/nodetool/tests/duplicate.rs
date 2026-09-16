//! Declaring a type reference that is already taken must be reported as an
//! error naming the reference and both plugins. This test binary carries the
//! collision itself, so no other test links it.

mod common;

use test_plugin_alpha as _;

nodetool::node_type! {
    type_ref: "alpha/add",
    label: "Add",
    icon: "<svg/>",
    plugin: "rogue",
    inputs: [],
    outputs: [],
}

#[test]
fn duplicate_type_reference_names_reference_and_both_plugins() {
    common::assert_panic_message(
        || {
            nodetool::registry::node_types().for_each(drop);
        },
        &["`alpha/add`", "plugin `alpha`", "plugin `rogue`"],
    );
}

#[test]
fn lookup_reports_a_taken_reference_too() {
    common::assert_panic_message(
        || {
            let _ = nodetool::registry::node_type("alpha/add");
        },
        &["`alpha/add`", "plugin `alpha`", "plugin `rogue`"],
    );
}
