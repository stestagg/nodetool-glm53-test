//! The JSON envelope and the message catalogue the editor speaks.
//!
//! Framing rules, settled and additive: a browser message carries a `type`
//! and an `id` the reply echoes; a reply is either the answer or an `error`
//! naming the problem; pushes ride the same envelope with no id. Unknown
//! message types and unknown fields are errors, never silently ignored
//! keys — a typo'd message should be visible. An error to a message too
//! malformed to carry an id has none.

use serde_json::{json, Map, Value};

use crate::graph::{GraphDefinition, Mapping, SCHEMA_VERSION};
use crate::registry;
use crate::{NodeType, Port};

/// The protocol version this server speaks; the greeting names it so a
/// mismatch is visible rather than silent.
pub const PROTOCOL_VERSION: u64 = 1;

/// The greeting pushed to every new connection: the versions this server
/// speaks.
pub fn greeting() -> String {
    serde_json::to_string(&json!({
        "type": "greeting",
        "protocol_version": PROTOCOL_VERSION,
        "schema_version": SCHEMA_VERSION,
    }))
    .expect("the greeting always serialises")
}

/// The node-type listing every linked plugin contributes, straight from the
/// registry and naming nothing specific to any node or plugin: the
/// palette's data source, its type references the keys every operation
/// names a node type by. Sorted by type reference, so every view sees the
/// same order.
pub fn node_type_listing() -> Vec<&'static NodeType> {
    let mut types: Vec<&'static NodeType> = registry::node_types().collect();
    types.sort_by_key(|node_type| node_type.type_ref);
    types
}

pub fn node_type_json(node_type: &NodeType) -> Value {
    json!({
        "type_ref": node_type.type_ref,
        "label": node_type.label,
        "icon": node_type.icon,
        "plugin": node_type.plugin,
        "sub_group": node_type.sub_group,
        "inputs": ports_json(node_type.inputs),
        "outputs": ports_json(node_type.outputs),
    })
}

fn ports_json(ports: &[Port]) -> Value {
    Value::Array(
        ports
            .iter()
            .map(|port| {
                json!({
                    "name": port.name,
                    "type_refs": port.type_refs,
                })
            })
            .collect(),
    )
}

/// The whole definition, serialised as JSON under the same schema the
/// files carry — parameters, edges, and metadata verbatim. Fails only when
/// the definition carries something JSON cannot, which the caller reports
/// instead of papering over.
pub fn definition_message(graph: &GraphDefinition) -> Result<String, serde_json::Error> {
    serde_json::to_string(&json!({ "type": "definition", "graph": graph }))
}

/// An error reply, echoing the id of the request it answers when it can.
pub fn error_reply(id: Option<Value>, message: &str) -> String {
    let mut reply = json!({ "type": "error", "error": message });
    if let Some(id) = id {
        reply["id"] = id;
    }
    reply.to_string()
}

/// Take one required string field out of a message's fields.
pub fn take_string(fields: &mut Map<String, Value>, name: &str) -> Result<String, String> {
    match fields.remove(name) {
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(format!("`{name}` must be a string")),
        None => Err(format!("missing field `{name}`")),
    }
}

/// A drop or resting position, as the browser measures it.
pub struct Position {
    pub x: serde_json::Number,
    pub y: serde_json::Number,
}

impl Position {
    /// The position as the `position` entry of a node's metadata, recorded
    /// verbatim: visual bookkeeping the engine never reads.
    pub fn metadata_entry(&self) -> Mapping {
        let mut position = Mapping::new();
        position.insert("x".into(), yaml_number(&self.x));
        position.insert("y".into(), yaml_number(&self.y));
        position
    }
}

/// A JSON number as YAML, keeping the integer kind when it has one, so a
/// recorded position reads the same in both serialisations.
fn yaml_number(number: &serde_json::Number) -> crate::graph::Value {
    if let Some(int) = number.as_i64() {
        crate::graph::Value::Number(int.into())
    } else {
        crate::graph::Value::Number(serde_yaml::Number::from(
            number.as_f64().unwrap_or(f64::NAN),
        ))
    }
}

/// Take one required position field: an object carrying exactly `x` and
/// `y`, both numbers.
pub fn take_position(fields: &mut Map<String, Value>, name: &str) -> Result<Position, String> {
    let value = fields
        .remove(name)
        .ok_or_else(|| format!("missing field `{name}`"))?;
    let Value::Object(position) = value else {
        return Err(format!("`{name}` must be an object with `x` and `y`"));
    };
    if position.len() != 2 {
        return Err(format!(
            "`{name}` carries an unknown field, expected exactly `x` and `y`"
        ));
    }
    let coordinate = |axis: &str| {
        position
            .get(axis)
            .ok_or_else(|| format!("`{name}.{axis}` is required and must be a number"))?
            .as_number()
            .cloned()
            .ok_or_else(|| format!("`{name}.{axis}` must be a number"))
    };
    Ok(Position {
        x: coordinate("x")?,
        y: coordinate("y")?,
    })
}

/// The fields left after a message's known ones are taken: any remaining
/// key is an unknown field, named rather than ignored.
pub fn done(fields: &Map<String, Value>) -> Result<(), String> {
    match fields.keys().next() {
        None => Ok(()),
        Some(key) => Err(format!("unknown field `{key}`")),
    }
}
