//! The listing's data-type fact, pinned as the file the editor UI's
//! contrast checks read. The colours live as declarations in Rust, flow
//! through the server's listing fact, and reach the browser as data — the
//! fixture is that data, held still so the browser side never restates a
//! colour. A declaration change that moves a colour fails here until the
//! fixture is regenerated, and the UI's contrast checks then judge the new
//! value against the surfaces it renders on.

use std::fs;

use nodetool::graph;
use nodetool::server::Editor;
use serde_json::Value;
use test_plugin_alpha as _;
use test_plugin_beta as _;

/// The pinned fact: `data_types` as the `list_node_types` reply carries it,
/// for the same plugin linkage the server tests use.
const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui/listing-fact.json");

fn listing_fact() -> Value {
    let editor = Editor::new(graph::GraphDefinition::empty(), None);
    let reply: Value =
        serde_json::from_str(&editor.handle(r#"{"id": 1, "type": "list_node_types"}"#))
            .expect("every reply is a JSON object");
    reply["data_types"].clone()
}

#[test]
fn the_listing_fact_matches_the_pinned_file() {
    let pretty = serde_json::to_string_pretty(&listing_fact()).expect("the fact serialises") + "\n";
    if std::env::var_os("NODETOOL_UPDATE_LISTING_FACT").is_some() {
        fs::write(FIXTURE, pretty).expect("the fixture path is writable");
        return;
    }
    let pinned = fs::read_to_string(FIXTURE).expect(
        "the listing fact fixture is missing; regenerate it with \
         NODETOOL_UPDATE_LISTING_FACT=1 cargo test --test listing_fact",
    );
    assert_eq!(
        pinned, pretty,
        "the listing fact drifted from the pinned fixture; regenerate with \
         NODETOOL_UPDATE_LISTING_FACT=1 cargo test --test listing_fact"
    );
}
