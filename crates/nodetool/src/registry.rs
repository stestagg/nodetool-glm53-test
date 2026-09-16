//! Reading back what the linked crates declared: node types and the data
//! type vocabulary.

use std::collections::HashMap;

use uuid::Uuid;

use crate::{DataType, NodeType};

/// The vocabulary of one binary — every node type and data type the linked
/// plugins declared — collected once and queried many times.
///
/// The free functions below walk the inventory on every call; a
/// [`Registry::collect`] snapshot does the walk once, so a caller that
/// compiles repeatedly — the compiler, the editor's recompile loop — never
/// rescans registrations. Collecting validates the registry the same way a
/// read does, panicking on taken names, ids, and references.
pub struct Registry {
    node_types: HashMap<&'static str, &'static NodeType>,
    data_types: HashMap<&'static str, &'static DataType>,
    data_types_by_id: HashMap<Uuid, &'static DataType>,
}

impl Registry {
    /// Gather the declarations of every linked plugin.
    pub fn collect() -> Registry {
        let node_types = node_types()
            .map(|node_type| (node_type.type_ref, node_type))
            .collect();
        let mut by_name = HashMap::new();
        let mut by_id = HashMap::new();
        for data_type in data_types() {
            by_name.insert(data_type.name, data_type);
            by_id.insert(data_type.id, data_type);
        }
        Registry {
            node_types,
            data_types: by_name,
            data_types_by_id: by_id,
        }
    }

    /// The node type declared under `type_ref`, if any.
    pub fn node_type(&self, type_ref: &str) -> Option<&'static NodeType> {
        self.node_types.get(type_ref).copied()
    }

    /// The data type registered under `name`, if any.
    pub fn data_type(&self, name: &str) -> Option<&'static DataType> {
        self.data_types.get(name).copied()
    }

    /// The data type registered under `id`, if any.
    pub fn data_type_by_id(&self, id: Uuid) -> Option<&'static DataType> {
        self.data_types_by_id.get(&id).copied()
    }
}

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
