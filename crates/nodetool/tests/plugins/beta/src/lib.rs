//! A flat test plugin: no sub-groups, a multi-reference port, a node type
//! with no inputs, a port reference the registry does not know, and data
//! types on both sides of the declared-both appearance rule.

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
    outputs: [ tick: "bool", haunt: "beta/ghost-type" ],
}

data_type! {
    id: uuid!("b3ba7ad1-6f2e-4c8a-9d50-1a7c5e3f8b62"),
    name: "beta/tinted",
    meta: [
        "color" => MetaValue::Str("#0e9488"),
        "shape" => MetaValue::Str("square"),
    ],
}

data_type! {
    id: uuid!("d08f0220-243b-4257-b552-d9ff4f2587d4"),
    name: "beta/hued",
    meta: [ "color" => MetaValue::Str("#0e9488") ],
}
