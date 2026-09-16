//! The versioned YAML graph file format: the one static document a headless
//! run loads and the visual editor opens, the same shape in both
//! experiences.
//!
//! A graph file is plain YAML, writable by hand in a text editor:
//!
//! ```yaml
//! schema_version: 1
//! name: circle to uppercase
//! nodes:
//!   - uuid: b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33
//!     type_ref: shapes/circle
//!     label: The circle
//!     parameters:
//!       radius: 2.5
//!     metadata:
//!       position: { x: 80, y: 120 }
//!   - uuid: 7c9e6679-7425-40de-944b-e07fc1f90ae7
//!     type_ref: text/uppercase
//! edges:
//!   - from: b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33
//!     from_port: shape
//!     to: 7c9e6679-7425-40de-944b-e07fc1f90ae7
//!     to_port: text
//! ```
//!
//! The document carries the [`SCHEMA_VERSION`] it was written for, an
//! optional name, the graph's nodes, and its edges. Each node is an
//! instance, not a type: its own uuid, the type reference of the node type
//! it instantiates, an optional user label overriding the type's default
//! label, parameter values for inputs fixed as literals, and a metadata
//! mapping for visual bookkeeping — layout position and the like — carried
//! verbatim; the engine never reads it. Edges run from an output to an
//! input: an input takes at most one upstream, an output may feed any
//! number of downstream inputs. An input's value comes from at most one of
//! two ways — an edge, or a parameter value written as a plain YAML scalar
//! (boolean, integer, float, or string).
//!
//! Loading is structural, and deliberately no more. The loader checks the
//! document's shape — well-formed YAML, nodes and edges where they belong,
//! edges pointing at nodes in the file, at most one upstream per input, no
//! unknown fields, a schema version this reader knows — and nothing else:
//! no node types need to be registered at all, and whether a referenced
//! type exists, whether ports line up, whether a connection's types are
//! compatible is compile time's business, so a file referencing types this
//! binary never linked still loads. What loading guarantees is honesty
//! about failure: a file that fails to load fails loudly and precisely, the
//! error naming the offending node, edge, or field and where it sits —
//! nothing guessed, defaulted, or silently dropped.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use serde::{Serialize, Serializer};
use uuid::Uuid;

pub use serde_yaml::{Mapping, Value};

/// The schema version this reader understands, and the version every dump
/// carries.
pub const SCHEMA_VERSION: u64 = 1;

/// A graph definition as a file carries it: node instances and the edges
/// between them.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphDefinition {
    /// The graph's name, if the file gives one.
    pub name: Option<String>,
    pub nodes: Vec<NodeInstance>,
    pub edges: Vec<Edge>,
}

/// One node instance. Not a type: an instance carries the type reference of
/// the node type it instantiates, plus everything that is this node's own.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeInstance {
    /// The instance's uuid, unique within the file.
    pub uuid: Uuid,
    /// The type reference naming which registered node type this
    /// instantiates. The loader never checks it against the registry.
    pub type_ref: String,
    /// A user-chosen label overriding the type's default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Values fixed for input ports as literals, by port name. Whether a
    /// name is a port of the type, and whether the literal suits it, is
    /// compile time's business.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,
    /// Visual bookkeeping — layout position and the like — carried
    /// verbatim; the engine never reads it.
    #[serde(skip_serializing_if = "Mapping::is_empty")]
    pub metadata: Mapping,
}

/// An edge from one node's output port to another node's input port. An
/// input takes at most one upstream; an output may feed any number of
/// downstream inputs.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Edge {
    pub from: Uuid,
    pub from_port: String,
    pub to: Uuid,
    pub to_port: String,
}

/// A parameter value: a plain YAML scalar of one of the four kinds.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ParameterValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterValue::Bool(value) => write!(f, "{value}"),
            ParameterValue::Int(value) => write!(f, "{value}"),
            ParameterValue::Float(value) => write!(f, "{value}"),
            ParameterValue::Str(value) => write!(f, "{value:?}"),
        }
    }
}

/// Where a load error sits: line and column for malformed YAML, an index
/// path into the document for shape errors — one a text editor can jump to.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadLocation {
    Position { line: usize, column: usize },
    Path(String),
}

impl fmt::Display for LoadLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadLocation::Position { line, column } => write!(f, "line {line}, column {column}"),
            LoadLocation::Path(path) => write!(f, "{path}"),
        }
    }
}

/// Why a graph file failed to load, naming the problem and where it sits.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadError {
    pub location: LoadLocation,
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

impl std::error::Error for LoadError {}

const DOCUMENT: &str = "document";
const NODES: &str = "nodes";
const EDGES: &str = "edges";

const DOCUMENT_FIELDS: &[&str] = &["schema_version", "name", "nodes", "edges"];
const NODE_FIELDS: &[&str] = &["uuid", "type_ref", "label", "parameters", "metadata"];
const EDGE_FIELDS: &[&str] = &["from", "from_port", "to", "to_port"];

/// Load a graph definition from YAML text.
///
/// Graph files are untrusted input at a parse boundary: a file that fails
/// to load fails with a precise [`LoadError`], never a panic.
pub fn load(text: &str) -> Result<GraphDefinition, LoadError> {
    let root = serde_yaml::from_str::<Value>(text).map_err(parse_error)?;
    load_document(&root)
}

/// Dump a graph definition back to YAML — the same format [`load`] reads,
/// so what one tool writes another reads back unchanged.
pub fn dump(graph: &GraphDefinition) -> String {
    serde_yaml::to_string(graph).expect("a graph definition always serialises")
}

impl Serialize for GraphDefinition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let len = 3 + usize::from(self.name.is_some());
        let mut map = serializer.serialize_map(Some(len))?;
        map.serialize_entry("schema_version", &SCHEMA_VERSION)?;
        if let Some(name) = &self.name {
            map.serialize_entry("name", name)?;
        }
        map.serialize_entry("nodes", &self.nodes)?;
        map.serialize_entry("edges", &self.edges)?;
        map.end()
    }
}

fn parse_error(error: serde_yaml::Error) -> LoadError {
    let message = error.to_string();
    match error.location() {
        Some(location) => LoadError {
            location: LoadLocation::Position {
                line: location.line(),
                column: location.column(),
            },
            message: without_position(&message, location),
        },
        None => LoadError {
            location: LoadLocation::Path(DOCUMENT.to_owned()),
            message,
        },
    }
}

/// serde_yaml renders the position inside its message; the structured
/// location carries it, so the repeated text is dropped where it appears.
fn without_position(message: &str, location: serde_yaml::Location) -> String {
    let position = format!(" at line {} column {}", location.line(), location.column());
    message.replace(&position, "")
}

fn load_document(root: &Value) -> Result<GraphDefinition, LoadError> {
    let fields = as_mapping(root, DOCUMENT)?;
    let mut schema_version = None;
    let mut name = None;
    let mut nodes = None;
    let mut edges = None;
    for (key, value) in fields {
        match key.as_str() {
            Some("schema_version") => schema_version = Some(value),
            Some("name") => name = Some(value),
            Some(NODES) => nodes = Some(value),
            Some(EDGES) => edges = Some(value),
            _ => {
                let text = key_text(key);
                return Err(unknown_field(&text, &text, DOCUMENT_FIELDS));
            }
        }
    }
    check_schema_version(
        schema_version.ok_or_else(|| err(DOCUMENT, "missing field `schema_version`"))?,
    )?;
    let name = match name {
        Some(value) => Some(as_str(value, "name")?.to_owned()),
        None => None,
    };
    let nodes = match nodes {
        Some(value) => load_nodes(value)?,
        None => return Err(err(DOCUMENT, "missing field `nodes`")),
    };
    let edges = match edges {
        Some(value) => load_edges(value)?,
        None => return Err(err(DOCUMENT, "missing field `edges`")),
    };
    cross_check(&nodes, &edges)?;
    Ok(GraphDefinition { name, nodes, edges })
}

fn check_schema_version(value: &Value) -> Result<(), LoadError> {
    let version = value.as_u64().ok_or_else(|| {
        let not = match value {
            Value::Number(_) => String::new(),
            other => format!(", not {}", kind(other)),
        };
        err(
            "schema_version",
            format!("the schema version must be a non-negative integer{not}"),
        )
    })?;
    if version != SCHEMA_VERSION {
        return Err(err(
            "schema_version",
            format!("unsupported schema version {version}; this reader understands version {SCHEMA_VERSION}"),
        ));
    }
    Ok(())
}

fn load_nodes(value: &Value) -> Result<Vec<NodeInstance>, LoadError> {
    let items = as_sequence(value, NODES)?;
    items
        .iter()
        .enumerate()
        .map(|(index, node)| load_node(node, index))
        .collect()
}

fn load_node(node: &Value, index: usize) -> Result<NodeInstance, LoadError> {
    let path = format!("{NODES}[{index}]");
    let fields = as_mapping(node, &path)?;
    let mut uuid = None;
    let mut type_ref = None;
    let mut label = None;
    let mut parameters = None;
    let mut metadata = None;
    for (key, value) in fields {
        match key.as_str() {
            Some("uuid") => uuid = Some(value),
            Some("type_ref") => type_ref = Some(value),
            Some("label") => label = Some(value),
            Some("parameters") => parameters = Some(value),
            Some("metadata") => metadata = Some(value),
            _ => {
                let text = key_text(key);
                return Err(unknown_field(&format!("{path}.{text}"), &text, NODE_FIELDS));
            }
        }
    }
    let uuid = load_uuid(
        uuid.ok_or_else(|| err(&path, "missing field `uuid`"))?,
        &format!("{path}.uuid"),
    )?;
    let type_ref = as_str(
        type_ref.ok_or_else(|| err(&path, "missing field `type_ref`"))?,
        &format!("{path}.type_ref"),
    )?
    .to_owned();
    let label = match label {
        Some(value) => Some(as_str(value, &format!("{path}.label"))?.to_owned()),
        None => None,
    };
    let parameters = match parameters {
        Some(value) => load_parameters(value, &format!("{path}.parameters"))?,
        None => BTreeMap::new(),
    };
    let metadata = match metadata {
        Some(value) => as_mapping(value, &format!("{path}.metadata"))?.clone(),
        None => Mapping::new(),
    };
    Ok(NodeInstance {
        uuid,
        type_ref,
        label,
        parameters,
        metadata,
    })
}

fn load_parameters(
    value: &Value,
    path: &str,
) -> Result<BTreeMap<String, ParameterValue>, LoadError> {
    let fields = as_mapping(value, path)?;
    let mut parameters = BTreeMap::new();
    for (key, value) in fields {
        let name = key.as_str().ok_or_else(|| {
            err(
                path,
                format!("the parameter name {} is not a string", key_text(key)),
            )
        })?;
        parameters.insert(
            name.to_owned(),
            load_parameter(value, &format!("{path}.{name}"))?,
        );
    }
    Ok(parameters)
}

fn load_parameter(value: &Value, path: &str) -> Result<ParameterValue, LoadError> {
    match value {
        Value::Bool(value) => Ok(ParameterValue::Bool(*value)),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                Ok(ParameterValue::Int(int))
            } else if number.is_f64() {
                Ok(ParameterValue::Float(number.as_f64().unwrap()))
            } else {
                Err(err(
                    path,
                    format!("a parameter value must be a plain scalar — boolean, integer, float, or string; {number} does not fit a signed integer"),
                ))
            }
        }
        Value::String(value) => Ok(ParameterValue::Str(value.clone())),
        other => Err(err(
            path,
            format!("a parameter value must be a plain scalar — boolean, integer, float, or string — not {}", kind(other)),
        )),
    }
}

fn load_edges(value: &Value) -> Result<Vec<Edge>, LoadError> {
    let items = as_sequence(value, EDGES)?;
    items
        .iter()
        .enumerate()
        .map(|(index, edge)| load_edge(edge, index))
        .collect()
}

fn load_edge(edge: &Value, index: usize) -> Result<Edge, LoadError> {
    let path = format!("{EDGES}[{index}]");
    let fields = as_mapping(edge, &path)?;
    let (mut from, mut from_port, mut to, mut to_port) = (None, None, None, None);
    for (key, value) in fields {
        match key.as_str() {
            Some("from") => from = Some(value),
            Some("from_port") => from_port = Some(value),
            Some("to") => to = Some(value),
            Some("to_port") => to_port = Some(value),
            _ => {
                let text = key_text(key);
                return Err(unknown_field(&format!("{path}.{text}"), &text, EDGE_FIELDS));
            }
        }
    }
    Ok(Edge {
        from: load_uuid(
            from.ok_or_else(|| err(&path, "missing field `from`"))?,
            &format!("{path}.from"),
        )?,
        from_port: as_str(
            from_port.ok_or_else(|| err(&path, "missing field `from_port`"))?,
            &format!("{path}.from_port"),
        )?
        .to_owned(),
        to: load_uuid(
            to.ok_or_else(|| err(&path, "missing field `to`"))?,
            &format!("{path}.to"),
        )?,
        to_port: as_str(
            to_port.ok_or_else(|| err(&path, "missing field `to_port`"))?,
            &format!("{path}.to_port"),
        )?
        .to_owned(),
    })
}

/// The document-wide cross-checks between the node list and the edge list:
/// uuids unique, edges landing on nodes in the file, at most one upstream
/// per input, an input not carrying both a parameter value and a
/// connection.
fn cross_check(nodes: &[NodeInstance], edges: &[Edge]) -> Result<(), LoadError> {
    let mut indexes = HashMap::<Uuid, usize>::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Some(first) = indexes.insert(node.uuid, index) {
            return Err(err(
                &format!("{NODES}[{index}]"),
                format!(
                    "duplicate node uuid {}; already used at nodes[{first}]",
                    node.uuid
                ),
            ));
        }
    }
    let mut connected = HashMap::<(Uuid, &str), usize>::new();
    for (index, edge) in edges.iter().enumerate() {
        let path = format!("{EDGES}[{index}]");
        for (uuid, end) in [(&edge.from, "from"), (&edge.to, "to")] {
            if !indexes.contains_key(uuid) {
                return Err(err(
                    &path,
                    format!("the edge's `{end}` node {uuid} is not defined in the file"),
                ));
            }
        }
        if let Some(first) = connected.insert((edge.to, edge.to_port.as_str()), index) {
            return Err(err(
                &path,
                format!(
                    "input `{}` of node {} receives more than one connection; edges[{first}] already feeds it",
                    edge.to_port, edge.to
                ),
            ));
        }
        if nodes[indexes[&edge.to]]
            .parameters
            .contains_key(&edge.to_port)
        {
            return Err(err(
                &path,
                format!(
                    "input `{}` of node {} holds a parameter value and receives a connection; an input carries one or the other",
                    edge.to_port, edge.to
                ),
            ));
        }
    }
    Ok(())
}

fn load_uuid(value: &Value, path: &str) -> Result<Uuid, LoadError> {
    let text = as_str(value, path)?;
    Uuid::parse_str(text).map_err(|_| err(path, format!("invalid uuid `{text}`")))
}

fn as_mapping<'v>(value: &'v Value, path: &str) -> Result<&'v Mapping, LoadError> {
    value
        .as_mapping()
        .ok_or_else(|| err(path, format!("must be a mapping, not {}", kind(value))))
}

fn as_sequence<'v>(value: &'v Value, path: &str) -> Result<&'v Vec<Value>, LoadError> {
    value
        .as_sequence()
        .ok_or_else(|| err(path, format!("must be a sequence, not {}", kind(value))))
}

fn as_str<'v>(value: &'v Value, path: &str) -> Result<&'v str, LoadError> {
    value
        .as_str()
        .ok_or_else(|| err(path, format!("must be a string, not {}", kind(value))))
}

/// What kind of YAML value this is, for error messages.
fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "an integer",
        Value::Number(_) => "a float",
        Value::String(_) => "a string",
        Value::Sequence(_) => "a sequence",
        Value::Mapping(_) => "a mapping",
        Value::Tagged(_) => "a tagged value",
    }
}

/// A mapping key as text, for error messages and paths. Value carries no
/// Display, so non-string keys render as their YAML form.
fn key_text(key: &Value) -> String {
    match key.as_str() {
        Some(text) => text.to_owned(),
        None => serde_yaml::to_string(key)
            .map(|text| text.trim_end().to_owned())
            .unwrap_or_else(|_| format!("{key:?}")),
    }
}

fn unknown_field(path: &str, key: &str, expected: &[&str]) -> LoadError {
    let expected = expected
        .iter()
        .map(|field| format!("`{field}`"))
        .collect::<Vec<_>>()
        .join(", ");
    err(
        path,
        format!("unknown field `{key}`, expected one of {expected}"),
    )
}

fn err(path: impl Into<String>, message: impl Into<String>) -> LoadError {
    LoadError {
        location: LoadLocation::Path(path.into()),
        message: message.into(),
    }
}
