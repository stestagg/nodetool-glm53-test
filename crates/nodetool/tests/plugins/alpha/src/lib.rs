//! Node type and data type declarations exercising the registry in nodetool's
//! tests.

use std::any::Any;

use nodetool::node_type;
use nodetool::scalars;
use nodetool::{data_type, uuid, MetaValue};

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
    inputs: [ parts: "String" ],
    outputs: [ text: "String" ],
}

data_type! {
    id: uuid!("d0e1f2a3-4b5c-4d6e-8f90-1a2b3c4d5e6f"),
    name: "alpha/ratio",
    conversions: [ scalars::F64 => ratio_as_f64 ],
    meta: [
        "min" => MetaValue::Float(0.0),
        "max" => MetaValue::Float(1.0),
    ],
}

/// The value a ratio node carries; opaque to core, this plugin's business.
pub struct Ratio(f64);

fn ratio_as_f64(value: &dyn Any) -> Option<Box<dyn Any>> {
    value
        .downcast_ref::<Ratio>()
        .map(|ratio| Box::new(ratio.0) as Box<dyn Any>)
}
