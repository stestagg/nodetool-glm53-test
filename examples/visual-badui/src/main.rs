//! The visual editor with one deliberately broken bundle linked in: the
//! badui plugin's node UI names a component contract version the editor
//! does not speak. Dropping the Bad UI widget renders it by the default
//! class — usable, never blank — and the report names the type. Every
//! other surface is `visual`'s.

use std::process::ExitCode;
use std::sync::Arc;

use nodetool::graph;
use nodetool::server::{read_definition, Editor, DEFAULT_ADDRESS};
use nodetool_fizzbuzz as _;
use nodetool_utility as _;
use plugin_badui as _;
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
    let listener = match tokio::net::TcpListener::bind(DEFAULT_ADDRESS).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "cannot bind {DEFAULT_ADDRESS}: {error}; another visual editor is probably already running"
            );
            return ExitCode::FAILURE;
        }
    };
    let address = listener.local_addr().expect("the listener is bound");
    println!("visual editor (with the deliberately unknown-contract bundle) on http://{address}");
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
