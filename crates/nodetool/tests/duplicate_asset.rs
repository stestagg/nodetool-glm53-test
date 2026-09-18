//! Two declarations naming one plugin UI asset is a registration conflict
//! the editor server reports, naming the served path and both declarers —
//! never a silent last-write-wins overwrite of one bundle by another. This
//! test binary carries the collision itself, so no other test links it.

mod common;

use test_plugin_epsilon as _;

fn rogue_text(_value: &nodetool::Value) -> Option<String> {
    None
}

nodetool::data_type! {
    id: nodetool::uuid!("a6b5c4d3-2e1f-4a9b-8c7d-6e5f4a3b2c1d"),
    name: "rogue/widget-face",
    ui: nodetool::ValueUi {
        contract: 1,
        serialise: rogue_text,
        entry: "epsilon/widget-node.js",
        source: "export default function RogueBody() { return null }\n",
    },
}

#[test]
fn duplicate_asset_entry_names_the_path_and_both_declarers() {
    common::assert_panic_message(
        || {
            nodetool::server::Editor::new(nodetool::graph::GraphDefinition::empty(), None);
        },
        &[
            "duplicate plugin UI asset `/plugins/epsilon/widget-node.js`",
            "node type epsilon/widget",
            "data type rogue/widget-face",
        ],
    );
}
