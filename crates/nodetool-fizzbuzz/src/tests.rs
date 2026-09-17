//! Unit-level checks that reach the nodes' private state — the output
//! node's collected vec, whose run-end retrieval seam is a later story's
//! settlement. The behaviour-driven coverage lives in the integration
//! tests.

use crate::Output;
use nodetool::behaviour::{drive, handoff, Input, Output as OutputPort};
use nodetool::scalars;
use nodetool::Value;

/// Drives the output behaviour with a scripted stream and returns what it
/// collected.
async fn collect(texts: &[&str]) -> Vec<String> {
    let (tx, rx) = handoff();
    let mut node = Output {
        collected: Vec::new(),
    };
    let mut inputs = [Input::new("text", rx)];
    let mut outputs: [OutputPort; 0] = [];
    let driven = tokio::spawn(async move {
        let outcome = drive(&mut node, &mut inputs, &mut outputs).await;
        (outcome, node)
    });
    for text in texts {
        tx.send(Value::new(scalars::STRING, text.to_string()))
            .await
            .expect("the stream takes the value");
    }
    drop(tx);
    let (outcome, node) = driven.await.expect("the node task ran");
    outcome.expect("the node completed");
    node.collected
}

#[tokio::test]
async fn the_output_collects_in_arrival_order() {
    let collected = collect(&["one", "two", "three"]).await;
    assert_eq!(collected, ["one", "two", "three"]);
}

#[tokio::test]
async fn an_output_with_no_values_collects_nothing() {
    let collected = collect(&[]).await;
    assert!(collected.is_empty());
}

#[tokio::test]
async fn the_output_completes_with_its_input() {
    let (tx, rx) = handoff();
    let mut node = Output {
        collected: Vec::new(),
    };
    let mut inputs = [Input::new("text", rx)];
    let mut outputs: [OutputPort; 0] = [];
    let driven = tokio::spawn(async move {
        let outcome = drive(&mut node, &mut inputs, &mut outputs).await;
        (outcome, node)
    });
    tx.send(Value::new(scalars::STRING, "only".to_string()))
        .await
        .expect("the stream takes the value");
    drop(tx);
    let (outcome, node) = driven.await.expect("the node task ran");
    outcome.expect("the node completed");
    assert_eq!(node.collected, ["only"]);
}
