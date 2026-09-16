//! Example nodetool plugin: shape nodes, sub-grouped by dimension, and the
//! custom data type their outputs carry.

use std::any::Any;

use nodetool::scalars;
use nodetool::{data_type, node_type, uuid, MetaValue};

data_type! {
    id: uuid!("3eb7d9c2-8f14-4a06-9b5d-2c6e1f8a4b70"),
    name: "shapes/shape",
    conversions: [ scalars::F64 => shape_area ],
    meta: [
        "color" => MetaValue::Str("#4a90d9"),
        "summary" => MetaValue::Str("2D and 3D shapes"),
    ],
}

/// The value carried on the shape outputs. Opaque to core: only this plugin
/// knows what a shape value looks like.
pub struct Shape {
    pub area: f64,
}

fn shape_area(value: &dyn Any) -> Option<Box<dyn Any>> {
    value
        .downcast_ref::<Shape>()
        .map(|shape| Box::new(shape.area) as Box<dyn Any>)
}

node_type! {
    type_ref: "shapes/circle",
    label: "Circle",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="6" fill="#4a90d9"/></svg>"##,
    plugin: "shapes",
    sub_group: "2d",
    inputs: [ radius: ["i32", "f64"] ],
    outputs: [ shape: "shapes/shape" ],
}

node_type! {
    type_ref: "shapes/rectangle",
    label: "Rectangle",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect x="2" y="3" width="12" height="10" fill="#4a90d9"/></svg>"##,
    plugin: "shapes",
    sub_group: "2d",
    inputs: [ width: "f64", height: "f64" ],
    outputs: [ shape: "shapes/shape" ],
}

node_type! {
    type_ref: "shapes/sphere",
    label: "Sphere",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="6" fill="#7b4ad9"/></svg>"##,
    plugin: "shapes",
    sub_group: "3d",
    inputs: [ radius: "f64" ],
    outputs: [ shape: "shapes/shape" ],
}
