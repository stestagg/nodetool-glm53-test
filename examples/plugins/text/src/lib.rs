//! Example nodetool plugin: flat text nodes, no sub-groups.

use nodetool::node_type;

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
    inputs: [ text: "String", separator: "String" ],
    outputs: [ parts: "String" ],
}
