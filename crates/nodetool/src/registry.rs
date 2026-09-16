//! Reading back the node types the linked plugins declared.

use std::collections::HashMap;

use crate::NodeType;

/// Every node type declared by the linked plugins, in unspecified order.
///
/// Every read walks the registry in full, so a taken type reference is always
/// reported: the error names the reference and both plugins.
pub fn node_types() -> impl Iterator<Item = &'static NodeType> {
    let mut seen = HashMap::<&'static str, &'static str>::new();
    inventory::iter::<NodeType>()
        .inspect(|node_type| {
            if let Some(first) = seen.get(node_type.type_ref) {
                panic!(
                    "duplicate node type reference `{}`: declared by both plugin `{}` and plugin `{}`",
                    node_type.type_ref, first, node_type.plugin
                );
            }
            seen.insert(node_type.type_ref, node_type.plugin);
        })
        .collect::<Vec<_>>()
        .into_iter()
}

/// The node type declared under `type_ref`, if any plugin declared it.
///
/// Reading the registry reports taken references; see [`node_types`].
pub fn node_type(type_ref: &str) -> Option<&'static NodeType> {
    node_types().find(|node_type| node_type.type_ref == type_ref)
}
