//! A flat test plugin: no sub-groups, a multi-reference port, and a node type
//! with no inputs.

use nodetool::node_type;

node_type! {
    type_ref: "beta/identity",
    label: "Identity",
    icon: "<svg/>",
    plugin: "beta",
    inputs: [ value: ["i32", "f64"] ],
    outputs: [ value: ["i32", "f64"] ],
}

node_type! {
    type_ref: "beta/tick",
    label: "Tick",
    icon: "<svg/>",
    plugin: "beta",
    inputs: [],
    outputs: [ tick: "bool" ],
}
