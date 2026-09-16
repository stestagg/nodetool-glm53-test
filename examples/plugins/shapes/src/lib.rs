//! Example nodetool plugin: shape nodes, sub-grouped by dimension.

use nodetool::node_type;

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
