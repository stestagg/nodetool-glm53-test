//! Example nodetool plugin: shape nodes, sub-grouped by dimension, and the
//! custom data type their outputs carry.

use nodetool::scalars;
use nodetool::{data_type, node_type, uuid, MetaValue, Value};

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

fn shape_area(value: &Value) -> Option<Value> {
    Some(Value::new(scalars::F64, value.get::<Shape>()?.area))
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
    type_ref: "shapes/polygon",
    label: "Polygon",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M8 2 13.7 6.1 11.5 12.6 4.5 12.6 2.3 6.1Z" fill="#4a90d9"/></svg>"##,
    plugin: "shapes",
    sub_group: "2d",
    inputs: [ sides: "i64" ],
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
