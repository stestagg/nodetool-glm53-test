//! Node type fixtures for the compiler tests: plain sources and sinks over
//! the base scalars, chosen so each type resolution and validation case —
//! exact matches across unions, every declared scalar conversion, the
//! plugin custom type's conversion, and each structural error — is
//! reachable from a small graph. The doubler is declared through the
//! authoring API, so the registry-to-driver path has a fixture too.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::node_type;
use nodetool::scalars;
use nodetool::Value;

struct Doubler;

#[async_trait]
impl Behaviour for Doubler {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        let value = io
            .input("value")
            .current()
            .and_then(|value| value.get::<i32>().copied())
            .expect("the input is declared i32 and the run is gated on it");
        io.output("value")
            .emit(Value::new(scalars::I32, value * 2))
            .await;
        Ok(Flow::Continue)
    }
}

fn doubler(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Doubler)
}

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

node_type! {
    type_ref: "gamma/doubler",
    label: "Doubler",
    icon: "<svg/>",
    plugin: "gamma",
    behaviour: doubler,
    inputs: [ value: "i32" ],
    outputs: [ value: "i32" ],
}
