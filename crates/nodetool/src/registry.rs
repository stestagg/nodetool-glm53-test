//! Reading back what the linked crates declared: node types and the data
//! type vocabulary.

use std::collections::HashMap;

use uuid::Uuid;

use crate::{DataType, NodeType};

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

/// Every data type the linked crates declared — the base scalars core ships,
/// plus every plugin's custom types — in unspecified order.
///
/// Every read walks the registry in full and validates it: a taken id or a
/// taken name is reported (the error names both declarations), as is a
/// conversion whose target never registered. Nothing is silently dropped.
pub fn data_types() -> impl Iterator<Item = &'static DataType> {
    let mut by_id = HashMap::<Uuid, &'static str>::new();
    let mut by_name = HashMap::<&'static str, Uuid>::new();
    let mut collected = Vec::<&'static DataType>::new();
    for data_type in inventory::iter::<DataType>() {
        if let Some(&first) = by_id.get(&data_type.id) {
            panic!(
                "duplicate data type id `{}`: registered by both `{first}` and `{}`",
                data_type.id, data_type.name
            );
        }
        if let Some(&first) = by_name.get(data_type.name) {
            panic!(
                "duplicate data type name `{}`: registered by both id `{first}` and id `{}`",
                data_type.name, data_type.id
            );
        }
        by_id.insert(data_type.id, data_type.name);
        by_name.insert(data_type.name, data_type.id);
        collected.push(data_type);
    }
    for declared in &collected {
        for conversion in declared.conversions {
            if !by_id.contains_key(&conversion.target) {
                panic!(
                    "data type `{}` declares a conversion to id `{}`, but no type with that id is registered",
                    declared.name, conversion.target
                );
            }
        }
    }
    collected.into_iter()
}

/// The data type registered under `name`, if any crate declared it.
///
/// Reading the registry validates it; see [`data_types`].
pub fn data_type(name: &str) -> Option<&'static DataType> {
    data_types().find(|data_type| data_type.name == name)
}

/// The data type registered under `id`, if any crate declared it.
///
/// Reading the registry validates it; see [`data_types`].
pub fn data_type_by_id(id: Uuid) -> Option<&'static DataType> {
    data_types().find(|data_type| data_type.id == id)
}
