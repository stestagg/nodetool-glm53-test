//! Node type fixtures for the compiler tests: plain sources and sinks over
//! the base scalars, chosen so each type resolution and validation case —
//! exact matches across unions, every declared scalar conversion, the
//! plugin custom type's conversion, and each structural error — is
//! reachable from a small graph.

use nodetool::node_type;

node_type! {
    type_ref: "gamma/int_source",
    label: "Int source",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [],
    outputs: [ value: "i32" ],
}

node_type! {
    type_ref: "gamma/int16_source",
    label: "Int16 source",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [],
    outputs: [ value: "i16" ],
}

node_type! {
    type_ref: "gamma/float32_source",
    label: "Float32 source",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [],
    outputs: [ value: "f32" ],
}

node_type! {
    type_ref: "gamma/ratio_source",
    label: "Ratio source",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [],
    outputs: [ value: "alpha/ratio" ],
}

node_type! {
    type_ref: "gamma/int_sink",
    label: "Int sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ value: "i32" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/float64_sink",
    label: "Float64 sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ value: "f64" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/string_sink",
    label: "String sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ text: "String" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/bool_sink",
    label: "Bool sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ flag: "bool" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/small_sink",
    label: "Small sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ value: "i8" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/unsigned_sink",
    label: "Unsigned sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ value: "u64" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/float32_sink",
    label: "Float32 sink",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ value: "f32" ],
    outputs: [],
}

node_type! {
    type_ref: "gamma/passthrough",
    label: "Passthrough",
    icon: "<svg/>",
    plugin: "gamma",
    inputs: [ value: "i32" ],
    outputs: [ value: "i32" ],
}
