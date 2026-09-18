//! The JSON envelope and the message catalogue the editor speaks.
//!
//! Framing rules, settled and additive: a browser message carries a `type`
//! and an `id` the reply echoes; a reply is either the answer or an `error`
//! naming the problem; pushes ride the same envelope with no id. Unknown
//! message types and unknown fields are errors, never silently ignored
//! keys — a typo'd message should be visible. An error to a message too
//! malformed to carry an id has none.

use serde_json::{json, Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

use super::assets::PLUGIN_PREFIX;
use super::bridge::RunDisplay;
use super::run::{Outcome, RunState};
use crate::engine::{Event, RunOutcome};
use crate::graph::{GraphDefinition, Mapping, ParameterValue, SCHEMA_VERSION};
use crate::registry;
use crate::{DataType, MetaValue, NodeType, Port};

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

/// Every linked plugin's node types, straight from the registry — core
/// names no type or plugin of its own. The palette draws its list from
/// here, and every operation names a node type by one of these type
/// references. Sorted by type reference, so every view sees the same order.
pub fn node_type_listing() -> Vec<&'static NodeType> {
    let mut types: Vec<&'static NodeType> = registry::node_types().collect();
    types.sort_by_key(|node_type| node_type.type_ref);
    types
}

/// The listing's base-scalar fact: per registered type reference, whether
/// the type is one of core's base scalars. One flat fact the browser's
/// field rule reads — computed here, where core's own scalar set is
/// known, so the browser hardcodes no scalar names of its own. A type
/// reference the registry does not know is absent, which reads as
/// not-a-base-scalar.
pub fn base_scalars() -> Map<String, Value> {
    let mut facts = Map::new();
    for data_type in registry::data_types() {
        facts.insert(
            data_type.name.to_owned(),
            Value::Bool(crate::scalars::is_base_scalar(data_type.id)),
        );
    }
    facts
}

/// The neutral pair: the colour and shape a port or wire renders when no
/// declared appearance informs it — a union-declared port, a reference the
/// registry does not know, a type that does not declare both. The browser
/// carries the same pair as its own neutral, so there is one neutral
/// everywhere, and it clears the editor's 3:1 edge floor against both the
/// canvas a wire sits on and the node surface a port dot sits on.
const NEUTRAL_COLOR: &str = "#6b7d8f";
const NEUTRAL_SHAPE: &str = "circle";

/// The listing's data-type fact: per data type reference — every name the
/// registry knows, and every reference the listed node types' ports
/// declare — the colour and shape its declaration gives it, and, when the
/// type declares custom value UI, the ui fact beside them. A type that
/// does not declare both a colour and a shape, and a reference the
/// registry does not know, get the neutral pair; a type with no declared
/// value UI carries no ui fact. Composed here, where the declarations
/// live, so the browser holds no colour, shape, or asset table of its
/// own: it renders the colour verbatim and takes the neutral for a shape
/// name outside the small set it draws.
pub fn data_type_facts(listing: &[&NodeType]) -> Map<String, Value> {
    let declared: HashMap<&str, &DataType> = registry::data_types()
        .map(|data_type| (data_type.name, data_type))
        .collect();
    let mut references: Vec<&str> = declared.keys().copied().collect();
    for node_type in listing {
        for port in node_type.inputs.iter().chain(node_type.outputs.iter()) {
            for reference in port.type_refs {
                if !declared.contains_key(reference) && !references.contains(reference) {
                    references.push(reference);
                }
            }
        }
    }
    references.sort_unstable();
    references
        .into_iter()
        .map(|reference| (reference.to_owned(), appearance(reference, &declared)))
        .collect()
}

/// One reference's colour and shape, and — when the type declares custom
/// value UI — the ui fact beside them: the declared pair when the type
/// declares both, the neutral pair otherwise. Nothing else about the type
/// is read, and nothing in core switches on either — the facts are
/// presentation the browser renders.
fn appearance(reference: &str, declared: &HashMap<&str, &DataType>) -> Value {
    let data_type = declared.get(reference).copied();
    let pair = data_type.and_then(|data_type| {
        Some(json!({
            "color": declared_str(data_type, "color")?,
            "shape": declared_str(data_type, "shape")?,
        }))
    });
    let mut fact =
        pair.unwrap_or_else(|| json!({ "color": NEUTRAL_COLOR, "shape": NEUTRAL_SHAPE }));
    if let Some(ui) = data_type.and_then(|data_type| data_type.ui) {
        fact["ui"] = ui_fact(ui.contract, ui.entry);
    }
    fact
}

fn declared_str(data_type: &DataType, key: &str) -> Option<&'static str> {
    match data_type.meta.iter().find(|(name, _)| *name == key) {
        Some((_, MetaValue::Str(value))) => Some(*value),
        _ => None,
    }
}

/// The listing's ui fact for one declared bundle: the entry asset's served
/// path — composed from the one per-plugin prefix the server serves under,
/// so the advertised path and the served path are the same composition —
/// and the component contract version the bundle names. Absent when
/// nothing is declared — no fact, no custom UI.
fn ui_fact(contract: u64, entry: &str) -> Value {
    json!({ "entry": format!("{PLUGIN_PREFIX}{entry}"), "contract": contract })
}

pub fn node_type_json(node_type: &NodeType) -> Value {
    let mut fact = json!({
        "type_ref": node_type.type_ref,
        "label": node_type.label,
        "icon": node_type.icon,
        "plugin": node_type.plugin,
        "sub_group": node_type.sub_group,
        "inputs": ports_json(node_type.inputs),
        "outputs": ports_json(node_type.outputs),
    });
    if let Some(ui) = node_type.ui {
        fact["ui"] = ui_fact(ui.contract, ui.entry);
    }
    fact
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

/// The file state that travels beside the definition: the file being
/// edited — none while the graph is untitled — and whether the held
/// definition has changed since it was last opened or saved. Every
/// connection renders its chrome from here, so no tab keeps file
/// bookkeeping of its own.
pub fn file_state(file: Option<&str>, dirty: bool) -> Value {
    json!({ "path": file, "dirty": dirty })
}

/// The run state that travels beside the definition, like the file state:
/// whether a run is on and, when one is not, how the last one ended —
/// the outcome, with what failed and at which node named. Every
/// connection renders its run control and the editing lock from here, so
/// no tab keeps run bookkeeping of its own.
pub fn run_state(run: &RunState) -> Value {
    let (running, outcome) = match run {
        RunState::Running { .. } => (true, None),
        RunState::Idle { outcome } => (false, outcome.as_ref()),
    };
    let failure = match outcome {
        Some(Outcome::Failed { error, node }) => Some((error.as_str(), *node)),
        _ => None,
    };
    json!({
        "running": running,
        "outcome": outcome.map(Outcome::name),
        "error": failure.map(|(error, _)| error),
        "node": failure.and_then(|(_, node)| node).map(|uuid| uuid.to_string()),
    })
}

/// The whole definition with the file state, the run state, and the
/// problems beside it — the message every connection renders the editor
/// from. The graph is serialized first and the message composed from that
/// value, so the failure the `Result` declares — the definition carrying
/// something JSON cannot, which the caller reports instead of papering
/// over — happens here rather than as a panic inside the composition. The
/// run display is not beside them: it travels as its own push, so a tab
/// joining mid-run receives it in the same ordered stream every later
/// status change and emission rides.
pub fn definition_message(
    graph: &GraphDefinition,
    file: Option<&str>,
    dirty: bool,
    run: &RunState,
    problems: &[crate::compile::Problem],
) -> Result<String, serde_json::Error> {
    let graph = serde_json::to_value(graph)?;
    serde_json::to_string(&json!({
        "type": "definition",
        "graph": graph,
        "file": file_state(file, dirty),
        "run": run_state(run),
        "problems": problems_state(problems),
    }))
}

/// The problems state that travels beside the definition: each problem's
/// message as compile writes it, and the node instances the problem
/// names. The canvas marks read this structure — never a parsing of the
/// messages — so a mark sits where compile says the problem lives.
pub fn problems_state(problems: &[crate::compile::Problem]) -> Value {
    Value::Array(
        problems
            .iter()
            .map(|problem| {
                json!({
                    "message": problem.message,
                    "nodes": problem.nodes.iter().map(|uuid| uuid.to_string()).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// The file state pushed alone, for a change that leaves the definition
/// untouched — a save.
pub fn file_message(file: Option<&str>, dirty: bool) -> String {
    serde_json::to_string(&json!({ "type": "file", "path": file, "dirty": dirty }))
        .expect("the file state always serialises")
}

/// The run state pushed alone, for a change that leaves the definition
/// untouched — a run starting or ending.
pub fn run_message(run: &RunState) -> String {
    let mut message = run_state(run);
    message["type"] = json!("run");
    serde_json::to_string(&message).expect("the run state always serialises")
}

/// A derived node status pushed alone: the bridge's one transition table,
/// spoken as state the canvas renders without deriving anything of its
/// own.
pub fn node_status_message(node: Uuid, status: &str) -> String {
    serde_json::to_string(&json!({
        "type": "node_status",
        "node": node,
        "status": status,
    }))
    .expect("a node status always serialises")
}

/// The run display pushed alone: the per-node statuses and latest values a
/// connecting tab is given, riding the one ordered channel every later
/// status change and emission rides. Pushes are sent under the session
/// lock, so a change the bridge derives later can never overtake the
/// snapshot a tab joins with — the stale-canvas-overwritten-by-a-push race
/// a reply-carried snapshot could not rule out.
pub fn run_display_message(display: &RunDisplay) -> String {
    let mut message = display.snapshot();
    message["type"] = json!("run_display");
    serde_json::to_string(&message).expect("the run display always serialises")
}

/// An engine event forwarded as it occurred — every event, values
/// included, no sampling or thinning; one event model crosses the bridge
/// untouched. A node event names the instance uuid, an emission names its
/// port and carries the value's browser-renderable text when core can
/// render one (a plugin custom type crosses with no invented content),
/// and run finished names its outcome. Pushed with no id, like every push.
pub fn run_event_message(event: &Event, value: Option<&str>) -> String {
    let mut message = json!({ "type": "run_event" });
    match event {
        Event::RunStarted => message["event"] = json!("run_started"),
        Event::NodeStarted { node } => {
            message["event"] = json!("node_started");
            message["node"] = json!(node.uuid);
        }
        Event::Emitted { node, port, .. } => {
            message["event"] = json!("emitted");
            message["node"] = json!(node.uuid);
            message["port"] = json!(port);
            if let Some(text) = value {
                message["value"] = json!(text);
            }
        }
        Event::NodeCompleted { node } => {
            message["event"] = json!("node_completed");
            message["node"] = json!(node.uuid);
        }
        Event::NodeFailed { node, error } => {
            message["event"] = json!("node_failed");
            message["node"] = json!(node.uuid);
            message["error"] = json!(error);
        }
        Event::RunFinished { outcome } => {
            message["event"] = json!("run_finished");
            message["outcome"] = json!(match outcome {
                RunOutcome::Complete => "completed",
                RunOutcome::Stopped => "stopped",
                RunOutcome::Failed { .. } => "failed",
            });
        }
    }
    serde_json::to_string(&message).expect("a forwarded event always serialises")
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

/// Take one optional string field out of a message's fields; absent
/// means the field was not sent.
pub fn take_optional_string(
    fields: &mut Map<String, Value>,
    name: &str,
) -> Result<Option<String>, String> {
    match fields.remove(name) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(format!("`{name}` must be a string")),
    }
}

/// Take one required uuid field out of a message's fields.
pub fn take_uuid(fields: &mut Map<String, Value>, name: &str) -> Result<Uuid, String> {
    let text = take_string(fields, name)?;
    Uuid::parse_str(&text).map_err(|_| format!("`{name}` must be a uuid"))
}

/// Take one optional parameter commit out of a message's fields. Absent
/// means the edit clears — an unset input is unset. Present, it is the
/// typed text carried as a string, and the server reads it by the file
/// format's own reading ([`crate::graph::read_parameter_text`]), so what
/// a commit stores is exactly what a hand-written file stores — the
/// browser hardcodes no scalar rules of its own.
pub fn take_parameter(
    fields: &mut Map<String, Value>,
    name: &str,
) -> Result<Option<ParameterValue>, String> {
    let text = match fields.remove(name) {
        None => return Ok(None),
        Some(Value::String(text)) => text,
        Some(_) => return Err(format!("`{name}` must be a string")),
    };
    crate::graph::read_parameter_text(&text)
        .map(Some)
        .map_err(|message| format!("`{name}` {message}"))
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
