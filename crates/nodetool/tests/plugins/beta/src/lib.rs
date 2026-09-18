//! A flat test plugin: no sub-groups, a multi-reference port, a node type
//! with no inputs, a custom type with appearance metadata, and a port
//! reference the registry does not know.

use nodetool::node_type;
use nodetool::{data_type, uuid, MetaValue};

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

node_type! {
    type_ref: "beta/ghost",
    label: "Ghost",
    icon: "<svg/>",
    plugin: "beta",
    inputs: [],
    outputs: [ haunt: "beta/ghost-type" ],
}

data_type! {
    id: uuid!("b3ba7ad1-6f2e-4c8a-9d50-1a7c5e3f8b62"),
    name: "beta/tinted",
    meta: [
        "color" => MetaValue::Str("#0e9488"),
        "shape" => MetaValue::Str("square"),
    ],
}
