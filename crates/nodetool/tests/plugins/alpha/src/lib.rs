//! Node type declarations exercising the registry in nodetool's tests.

use nodetool::node_type;

node_type! {
    type_ref: "alpha/add",
    label: "Add",
    icon: "<svg/>",
    plugin: "alpha",
    sub_group: "math",
    inputs: [ a: "i32", b: "i32" ],
    outputs: [ sum: "i32" ],
}

node_type! {
    type_ref: "alpha/concat",
    label: "Concat",
    icon: "<svg/>",
    plugin: "alpha",
    sub_group: "text",
    inputs: [ parts: "string" ],
    outputs: [ text: "string" ],
}
