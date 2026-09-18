//! Example nodetool plugin, deliberately broken once: its node UI names a
//! component contract version the editor does not speak. Linked into
//! `visual-badui` for the one deliberate look at the fallback: the node
//! still renders by the default class, and the report names the type.

use nodetool::{node_type, NodeUi};

node_type! {
    type_ref: "badui/widget",
    label: "Bad UI widget",
    icon: r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect x="3" y="3" width="10" height="10" fill="#a83a3e"/></svg>"##,
    plugin: "badui",
    ui: NodeUi {
        contract: 99,
        entry: "badui/widget-node.js",
        source: "export default function WidgetNode() { return null }\n",
    },
    inputs: [],
    outputs: [],
}
