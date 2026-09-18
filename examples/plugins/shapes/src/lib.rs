//! Example nodetool plugin: shape nodes, sub-grouped by dimension, and the
//! custom data type their outputs carry. The type declares its value UI —
//! the serialiser its values cross through and the bundle rendering that
//! form — and the stage node declares the node UI that renders its body in
//! the editor; both bundles live in `ui/`, embedded here at build time.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::scalars;
use nodetool::{data_type, node_type, uuid, MetaValue, NodeUi, Uuid, Value, ValueUi};

/// The id of the custom type this plugin owns.
pub const SHAPE: Uuid = uuid!("3eb7d9c2-8f14-4a06-9b5d-2c6e1f8a4b70");

data_type! {
    id: SHAPE,
    name: "shapes/shape",
    conversions: [ scalars::F64 => shape_area ],
    meta: [
        "color" => MetaValue::Str("#4a90d9"),
        "shape" => MetaValue::Str("circle"),
        "summary" => MetaValue::Str("2D and 3D shapes"),
    ],
    ui: ValueUi {
        contract: 1,
        serialise: shape_text,
        entry: "shapes/shape-value.js",
        source: include_str!("../ui/shape-value.js"),
    },
}

/// The value carried on the shape outputs. Opaque to core: only this plugin
/// knows what a shape value looks like.
#[derive(Clone, Copy)]
pub struct Shape {
    pub area: f64,
}

fn shape_area(value: &Value) -> Option<Value> {
    Some(Value::new(scalars::F64, value.get::<Shape>()?.area))
}

/// The shape's browser form: the serialiser the type's values cross
/// through, the text the value UI renders and any node UI reads.
fn shape_text(value: &Value) -> Option<String> {
    Some(format!("area {:.2}", value.get::<Shape>()?.area))
}

/// A shape source: each side length's arrival fires one emission of the
/// shape its square would cover. The default-rendered emitter the custom
/// value UI is watched at.
struct Source;

#[async_trait]
impl Behaviour for Source {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let side = io
            .input("side")
            .current()
            .and_then(|value| value.get::<f64>().copied())
            .expect("the run is gated on this input's first value");
        io.output("shape")
            .emit(Value::new(SHAPE, Shape { area: side * side }))
            .await;
        Ok(Flow::Continue)
    }
}

fn source_behaviour(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Source)
}

/// A stage in the stream: each shape arrival passes through, so the value
/// wired in is the value the stage's own UI displays — its output's latest,
/// serialised, riding the contract beside the label, the parameters, and
/// the marks.
struct Stage;

#[async_trait]
impl Behaviour for Stage {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // Each shape arrival re-emits; the note's own arrival merely fills
        // its held value — nothing new to stage.
        let Trigger::Arrival("shape") = trigger else {
            return Ok(Flow::Continue);
        };
        let shape = io
            .input("shape")
            .current()
            .and_then(|value| value.get::<Shape>().cloned())
            .expect("the run is gated on this input's first value");
        io.output("shape").emit(Value::new(SHAPE, shape)).await;
        Ok(Flow::Continue)
    }
}

fn stage_behaviour(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Stage)
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

node_type! {
    type_ref: "shapes/source",
    label: "Shape source",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect x="2" y="2" width="12" height="12" fill="none" stroke="#4a90d9" stroke-width="1.5"/><circle cx="8" cy="8" r="3" fill="#4a90d9"/></svg>"##,
    plugin: "shapes",
    behaviour: source_behaviour,
    inputs: [ side: "f64" ],
    outputs: [ shape: "shapes/shape" ],
}

node_type! {
    type_ref: "shapes/stage",
    label: "Shape stage",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><circle cx="8" cy="8" r="6" fill="none" stroke="#4a90d9" stroke-width="2"/></svg>"##,
    plugin: "shapes",
    behaviour: stage_behaviour,
    ui: NodeUi {
        contract: 1,
        entry: "shapes/stage-node.js",
        source: include_str!("../ui/stage-node.js"),
    },
    inputs: [ shape: "shapes/shape", note: "String" ],
    outputs: [ shape: "shapes/shape" ],
}
