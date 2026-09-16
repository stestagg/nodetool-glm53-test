//! Runs one node with real behaviour, straight from the linked plugins: the
//! text plugin's split node is looked up in the registry, its declared
//! behaviour built through the authoring API, and the node driven with two
//! scripted input streams — `text` streaming a few strings, `separator`
//! yielding a single value and then completing, the constant case. Each
//! emitted value prints as it arrives, then the node's completion.

use plugin_shapes as _;
use plugin_text as _;

use nodetool::behaviour::{drive, handoff, Input, Output};
use nodetool::registry::Registry;
use nodetool::scalars;
use nodetool::Value;

#[tokio::main]
async fn main() {
    let registry = Registry::collect();
    let node_type = registry
        .node_type("text/split")
        .expect("the text plugin declares text/split");
    let mut behaviour = (node_type
        .behaviour
        .expect("text/split is declared through the authoring API"))();

    let (text_tx, text_rx) = handoff();
    let (separator_tx, separator_rx) = handoff();
    let (parts_tx, mut parts_rx) = handoff();

    let node = tokio::spawn(async move {
        let mut inputs = [
            Input::new("text", text_rx),
            Input::new("separator", separator_rx),
        ];
        let mut outputs = [Output::new("parts")];
        outputs[0].connect(parts_tx);
        drive(behaviour.as_mut(), &mut inputs, &mut outputs).await
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
