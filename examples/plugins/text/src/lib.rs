//! Example nodetool plugin: flat text nodes, no sub-groups. The split node
//! is declared through the authoring API — descriptor and behaviour
//! together.

use nodetool::async_trait;

use nodetool::behaviour::{Behaviour, Error, Flow, Io, Trigger};
use nodetool::scalars;
use nodetool::{node_type, Value};

struct Split;

#[async_trait]
impl Behaviour for Split {
    async fn process(&mut self, _trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        // Every run fires after the gate, so each input holds a value; the
        // separator's single value survives its stream's end.
        let text = io
            .input("text")
            .current()
            .expect("the run is gated on `text`'s first value")
            .get::<String>()
            .cloned()
            .expect("`text` holds its declared String");
        let separator = io
            .input("separator")
            .current()
            .expect("the run is gated on `separator`'s first value")
            .get::<String>()
            .cloned()
            .expect("`separator` holds its declared String");
        for part in text.split(separator.as_str()) {
            io.output("parts")
                .emit(Value::new(scalars::STRING, part.to_owned()))
                .await;
        }
        Ok(Flow::Continue)
    }
}

fn split() -> Box<dyn Behaviour> {
    Box::new(Split)
}

node_type! {
    type_ref: "text/uppercase",
    label: "Uppercase",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><text x="3" y="12" font-size="12" fill="#333">A</text></svg>"##,
    plugin: "text",
    inputs: [ text: "String" ],
    outputs: [ text: "String" ],
}

node_type! {
    type_ref: "text/split",
    label: "Split",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><path d="M8 2v12M2 8h12" stroke="#333" stroke-width="2"/></svg>"##,
    plugin: "text",
    behaviour: split,
    inputs: [ text: "String", separator: "String" ],
    outputs: [ parts: "String" ],
}
