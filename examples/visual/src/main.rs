//! The visual path from a terminal: start the editor server and print the
//! address. Opening the served address shows the editor — the palette of
//! every node type the linked plugins contribute on the left, the canvas
//! beside it — and the graph the server holds. Edits land server-side over
//! the websocket, so a reload or a second tab shows the same graph.
//!
//! Launched on a graph file path the editor opens already showing that
//! graph, the file read through the editor's one file reading — the story
//! 03 loader, the same file the headless run takes. A path that cannot be
//! read or loaded prints the fault and exits non-zero. Without a path the
//! editor starts on an empty, untitled canvas.
//!
//! The server binds loopback on its default address; this is a local tool.

use std::process::ExitCode;
use std::sync::Arc;

use nodetool::graph;
use nodetool::server::{read_definition, Editor};
use nodetool_utility as _;
use plugin_shapes as _;
use plugin_text as _;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let file = std::env::args().nth(1);
    let definition = match file.as_deref().map(read_definition) {
        Some(Ok(definition)) => definition,
        Some(Err(error)) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
        None => graph::GraphDefinition::empty(),
    };
    let listener = match tokio::net::TcpListener::bind(nodetool::server::DEFAULT_ADDRESS).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let address = listener.local_addr().expect("the listener is bound");
    println!("visual editor on http://{address}");
    match Arc::new(Editor::new(definition, file))
        .serve(listener)
        .await
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
