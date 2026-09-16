//! Reading back the node types the linked plugins declared.

use std::collections::HashMap;

use crate::NodeType;

/// Every node type declared by the linked plugins, in registration order.
///
/// Panics if two plugins declare the same type reference: the error names the
/// reference and both plugins.
pub fn node_types() -> impl Iterator<Item = &'static NodeType> {
    let mut seen = HashMap::<&'static str, &'static str>::new();
    inventory::iter::<NodeType>().inspect(move |node_type| {
        if let Some(first) = seen.get(node_type.type_ref) {
            panic!(
                "duplicate node type reference `{}`: declared by both plugin `{}` and plugin `{}`",
                node_type.type_ref, first, node_type.plugin
            );
        }
        seen.insert(node_type.type_ref, node_type.plugin);
    })
}

/// The node type declared under `type_ref`, if any plugin declared it.
pub fn node_type(type_ref: &str) -> Option<&'static NodeType> {
    // Full walk, not an early exit: a duplicate reference must be reported
    // even when the match is found before it.
    node_types()
        .filter(|node_type| node_type.type_ref == type_ref)
        .last()
}
