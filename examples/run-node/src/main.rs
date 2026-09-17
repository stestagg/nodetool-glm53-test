//! Runs one node with real behaviour, straight from the linked plugins: the
//! text plugin's split node is looked up in the registry, its declared
//! behaviour built through the authoring API, and the node driven with two
//! scripted input streams — `text` streaming a few strings, `separator`
//! yielding a single value and then completing, the constant case. Each run
//! prints the arrival that fired it, each emitted value prints as it
//! arrives, and the node's completion prints last: every line on the
//! terminal accountable to the stream event that caused it.

use plugin_shapes as _;
use plugin_text as _;

use nodetool::async_trait;
use nodetool::behaviour::{drive, handoff, Behaviour, Error, Flow, Input, Io, Output, Trigger};
use nodetool::registry::Registry;
use nodetool::scalars;
use nodetool::Value;

/// Prints what fired each run, then hands the run to the plugin behaviour —
/// the demo's one addition, so every emission below is accountable to the
/// arrival that caused it.
struct Attributed {
    inner: Box<dyn Behaviour>,
}

#[async_trait]
impl Behaviour for Attributed {
    async fn process(&mut self, trigger: Trigger, io: &mut Io<'_>) -> Result<Flow, Error> {
        match &trigger {
            Trigger::Start => println!("-- start"),
            Trigger::Arrival(name) => println!("-- arrival on '{name}'"),
        }
        let flow = self.inner.process(trigger, io).await?;
        // Let the consumer drain and print this run's burst before the next
        // run is attributed, so the grouping on screen is the grouping that
        // ran.
        tokio::task::yield_now().await;
        Ok(flow)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let registry = Registry::collect();
    let node_type = registry
        .node_type("text/split")
        .expect("the text plugin declares text/split");
    println!(
        "running {}: `text` streams two values, `separator` yields one then \
         completes, and every arrival on either input fires a run",
        node_type.type_ref
    );
    let mut behaviour = Attributed {
        inner: {
            let compiled = nodetool::compile::CompiledNode {
                node_type,
                label: node_type.label.to_owned(),
                parameters: Default::default(),
                families: Default::default(),
            };
            (node_type
                .behaviour
                .expect("text/split is declared through the authoring API"))(&compiled)
        },
    };

    let (text_tx, text_rx) = handoff();
    let (separator_tx, separator_rx) = handoff();
    let (parts_tx, mut parts_rx) = handoff();

    let node = tokio::spawn(async move {
        let mut inputs = [
            Input::new("text", text_rx),
            Input::new("separator", separator_rx),
        ];
        let mut outputs = [Output::new("parts")];
        outputs[0].connect(parts_tx, None);
        drive(&mut behaviour, &mut inputs, &mut outputs).await
    });

    tokio::spawn(async move {
        // A constant is just a stream that yields once and completes.
        separator_tx
            .send(Value::new(scalars::STRING, ", ".to_owned()))
            .await
            .expect("the separator stream takes the value");
        for text in ["alpha, beta, gamma", "one, one, one"] {
            text_tx
                .send(Value::new(scalars::STRING, text.to_owned()))
                .await
                .expect("the text stream takes the value");
        }
    });

    let printed = tokio::spawn(async move {
        while let Some(value) = parts_rx.recv().await {
            println!(
                "emitted {}",
                value
                    .get::<String>()
                    .expect("the parts output is declared String")
            );
        }
    });

    let outcome = node.await.expect("the node task ran to its end");
    printed.await.expect("the consumer ran to its end");
    match outcome {
        Ok(()) => println!("the node completed"),
        Err(error) => println!("the node failed: {error}"),
    }
}
