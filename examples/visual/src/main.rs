//! The visual path from a terminal: start the editor server and print the
//! address. Opening the served address shows the editor — the palette of
//! every node type the linked plugins contribute on the left, the canvas
//! beside it — and the graph the server holds. Edits land server-side over
//! the websocket, so a reload or a second tab shows the same graph.
//!
//! The server binds loopback on its default address; this is a local tool.

use std::sync::Arc;

use nodetool::graph;
use nodetool::server::Editor;
use nodetool_utility as _;
use plugin_shapes as _;
use plugin_text as _;

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(nodetool::server::DEFAULT_ADDRESS).await?;
    let address = listener.local_addr()?;
    println!("visual editor on http://{address}");
    Arc::new(Editor::new(graph::GraphDefinition::empty()))
        .serve(listener)
        .await
}
