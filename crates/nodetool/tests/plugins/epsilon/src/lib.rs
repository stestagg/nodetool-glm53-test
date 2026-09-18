//! Node type and data type fixtures for the custom-UI tests: a runnable
//! source emitting the custom type — whose serialisation the bridge calls
//! generically at the crossing — and a node type whose UI is declared, so
//! the listing facts, the served assets, and the fallback model each have a
//! declaration to read. The data type declares the value UI; a second, the
//! `epsilon/lamp` below, stays undeclared, the contentless default beside
//! it in the same listing.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::{data_type, node_type, uuid, NodeUi, Uuid, Value, ValueUi};

/// The id of the custom type this plugin owns.
pub const TONE: Uuid = uuid!("5e1f2a7b-9c3d-4e8f-a0b1-c2d3e4f5a6b7");

/// The value a tone source emits; opaque to core, this plugin's business.
pub struct Tone {
    pub hz: f64,
}

/// The tone's browser form: the serialiser the type's values cross through.
pub fn tone_text(value: &Value) -> Option<String> {
    Some(format!("{} Hz", value.get::<Tone>()?.hz))
}

data_type! {
    id: TONE,
    name: "epsilon/tone",
    ui: ValueUi {
        contract: 1,
        serialise: tone_text,
        entry: "epsilon/tone-value.js",
        source: "export default function ToneValue({ value }) { return null }\n",
    },
}

/// A second custom type of the same plugin with no value UI declared: the
/// contentless default the serialiser-declared type is measured against.
pub struct Lamp;

data_type! {
    id: uuid!("8f2b4d6a-1c3e-4a5b-9d0f-e7a8b9c0d1e2"),
    name: "epsilon/lamp",
}

struct Source;

#[async_trait]
impl Behaviour for Source {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        io.output("tone")
            .emit(Value::new(TONE, Tone { hz: 440.0 }))
            .await;
        Ok(Flow::Complete)
    }
}

fn source(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(Source)
}

struct LampSource;

#[async_trait]
impl Behaviour for LampSource {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        io.output("lamp")
            .emit(Value::new(
                uuid!("8f2b4d6a-1c3e-4a5b-9d0f-e7a8b9c0d1e2"),
                Lamp,
            ))
            .await;
        Ok(Flow::Complete)
    }
}

fn lamp_source(_compiled: &nodetool::compile::CompiledNode) -> Box<dyn Behaviour> {
    Box::new(LampSource)
}

node_type! {
    type_ref: "epsilon/source",
    label: "Tone source",
    icon: "<svg/>",
    plugin: "epsilon",
    behaviour: source,
    inputs: [],
    outputs: [ tone: "epsilon/tone" ],
}

node_type! {
    type_ref: "epsilon/lamp_source",
    label: "Lamp source",
    icon: "<svg/>",
    plugin: "epsilon",
    behaviour: lamp_source,
    inputs: [],
    outputs: [ lamp: "epsilon/lamp" ],
}

node_type! {
    type_ref: "epsilon/widget",
    label: "Widget",
    icon: "<svg/>",
    plugin: "epsilon",
    ui: NodeUi {
        contract: 1,
        entry: "epsilon/widget-node.js",
        source: "export default function WidgetBody() { return null }\n",
    },
    inputs: [ level: "f64" ],
    outputs: [],
}
