//! The versioned YAML graph file format: the one static document a headless
//! run loads and the visual editor opens, the same shape in both
//! experiences.
//!
//! A graph file is plain YAML, writable by hand in a text editor:
//!
//! ```yaml
//! schema_version: 2
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
//! The document carries the [`SCHEMA_VERSION`] it is written as, an
//! optional name, the graph's nodes, its edges, and — since version 2 — an
//! optional `groups` section. Each node is an
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
//! A group is a named, reusable node type the document itself defines: a
//! nested graph packaged with exposed ports.
//!
//! ```yaml
//! groups:
//!   - name: shout
//!     inputs:
//!       - name: text
//!         type_refs: [String]
//!         node: 3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b
//!         port: text
//!     outputs:
//!       - name: text
//!         type_refs: [String]
//!         node: 3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b
//!         port: text
//!     nodes:
//!       - uuid: 3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b
//!         type_ref: text/uppercase
//!         metadata:
//!           position: { x: 40, y: 20 }
//!     edges: []
//! ```
//!
//! The name is unique in the document: it is the type reference a group
//! instance carries, and the default label its instances render by. The
//! exposed ports are the interface — each a name, declared type references
//! (the same port vocabulary as any node type's, unions allowed), and the
//! inner node and port it binds, the edge it stood in for at packaging
//! time. The nested graph is nodes and edges in exactly the top-level
//! shapes, and it is closed: an inner edge lands only on the group's own
//! nodes, so everything crossing the boundary crosses as an exposed port.
//! An instance of the group is an ordinary node instance whose type
//! reference names it, carrying label, parameters, and metadata like any
//! node's; groups may instantiate other groups, and a group that,
//! directly or through others, instantiates itself is an error like a
//! graph cycle.
//!
//! Versions: this reader understands versions 1 and 2, and writes
//! [`SCHEMA_VERSION`]. A version 1 file means exactly what it always
//! meant — the version gates the shape, so a version 1 document carrying a
//! `groups` section is a load error naming the version, never a v1 file
//! silently gaining meaning. Anything [`load`] returns is written as
//! version 2, so saving a loaded version 1 file upgrades it — the one
//! migration the format promises, the version field being enough.
//!
//! Loading is structural, and deliberately no more. The loader checks the
//! document's shape — well-formed YAML, nodes and edges where they belong,
//! edges pointing at nodes in their own graph, at most one upstream per
//! input, no unknown fields, a schema version this reader knows — and,
//! for groups, only what is document-local: each name unique in the
//! document, an inner edge landing inside its own group, a binding naming
//! an inner node the group contains. Nothing else: no node types need to
//! be registered at all, and whether a referenced type exists, whether a
//! bound port lines up, whether the declared types bridge is compile
//! time's business, so a file referencing types this binary never linked
//! still loads. What loading guarantees is honesty
//! about failure: a file that fails to load fails loudly and precisely, the
//! error naming the offending node, edge, or field and where it sits —
//! nothing guessed, defaulted, or silently dropped.
//!
//! The load contract, stated plainly: `schema_version`, `nodes`, and
//! `edges` are required — write `edges: []` for a graph with no edges — and
//! the schema version must be a non-negative integer this reader supports.
//! A group carries `name`, `inputs`, `outputs`, `nodes`, and `edges` —
//! write `inputs: []` and `outputs: []` for a group with neither. Duplicate
//! YAML keys are rejected rather than silently resolved, and
//! parameter names must be strings. One known gap in the locations: a
//! duplicate key is reported at its mapping's start rather than at the
//! repeated key, though the message names the key either way.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use serde::Serialize;
use uuid::Uuid;

/// The YAML value types a node's metadata carries, so callers build it
/// without a direct serde_yaml dependency.
pub use serde_yaml::{Mapping, Value};

/// The schema version this reader writes: the newest it understands, and
/// the one anything [`load`] returns. Loading also accepts the versions
/// before it that carried no `groups` section.
pub const SCHEMA_VERSION: u64 = 2;

/// A graph definition, exactly as a file carries it: the schema version it
/// is written as, an optional name, the node instances, the edges between
/// them, and the groups they may instantiate.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GraphDefinition {
    /// The schema version the file is written as; [`SCHEMA_VERSION`] for
    /// anything [`load`] returns.
    pub schema_version: u64,
    /// The graph's name, if the file gives one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub nodes: Vec<NodeInstance>,
    pub edges: Vec<Edge>,
    /// The groups the document defines, whose instances the nodes carry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupDefinition>,
}

impl GraphDefinition {
    /// The graph a fresh editor starts from.
    pub fn empty() -> GraphDefinition {
        GraphDefinition {
            schema_version: SCHEMA_VERSION,
            name: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
        }
    }
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
    /// Values fixed for input ports as literals, by port name, and the
    /// option each of the type's declared choices holds, under the choice's
    /// name. Whether a name is a port or a choice of the type, and whether
    /// the literal suits it, is compile time's business.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, ParameterValue>,
    /// Visual bookkeeping — layout position and the like — carried
    /// verbatim; the engine never reads it.
    #[serde(skip_serializing_if = "Mapping::is_empty")]
    pub metadata: Mapping,
}

impl NodeInstance {
    /// What a message calls this node: the instance's label, else the
    /// label of the type it instantiates, else — for a type nothing
    /// declares — the type reference the file writes. The uuid is the
    /// internal address, never a name a user reads, so it is absent here;
    /// two nodes may well answer to the same name, and where text alone
    /// cannot tell them apart, a mark points at the one meant.
    pub fn name<'a>(&'a self, node_type: Option<&'a crate::NodeType>) -> &'a str {
        self.label
            .as_deref()
            .unwrap_or_else(|| node_type.map_or(self.type_ref.as_str(), |declared| declared.label))
    }
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

/// A group: a named, reusable node type the document itself defines — a
/// nested graph packaged with exposed ports. Its instances are ordinary
/// node instances whose type reference names the group.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GroupDefinition {
    /// The group's name, unique in the document: the type reference its
    /// instances carry, and the default label they render by.
    pub name: String,
    /// The exposed inputs: what outside wiring may feed.
    pub inputs: Vec<GroupPort>,
    /// The exposed outputs: what the inside may emit outward.
    pub outputs: Vec<GroupPort>,
    /// The group's inner nodes, in exactly the top-level shape.
    pub nodes: Vec<NodeInstance>,
    /// The group's inner edges, in exactly the top-level shape.
    pub edges: Vec<Edge>,
}

/// One exposed port: the interface a group shows the outside, bound to the
/// inner node and port whose place it took at packaging time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GroupPort {
    /// The port's name on the group instance.
    pub name: String,
    /// The declared type references — the same vocabulary as any node
    /// type's port, unions allowed.
    pub type_refs: Vec<String>,
    /// The inner node the port binds.
    pub node: Uuid,
    /// The inner port, on that node, the port binds.
    pub port: String,
}

/// A parameter value: a plain YAML scalar of one of the four kinds.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum ParameterValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

/// Equality here is the round trip's: two values are equal when they carry
/// the same kind and the same meaning — so NaN equals NaN.
impl PartialEq for ParameterValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Float(a), Self::Float(b)) => a == b || (a.is_nan() && b.is_nan()),
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParameterValue::Bool(value) => write!(f, "{value}"),
            ParameterValue::Int(value) => write!(f, "{value}"),
            // Debug keeps a decimal point or exponent, so a float's kind
            // shows: 3.0 never reads as the integer 3.
            ParameterValue::Float(value) => write!(f, "{value:?}"),
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
const GROUPS: &str = "groups";

const DOCUMENT_FIELDS: &[&str] = &["schema_version", "name", "nodes", "edges", "groups"];
const NODE_FIELDS: &[&str] = &["uuid", "type_ref", "label", "parameters", "metadata"];
const EDGE_FIELDS: &[&str] = &["from", "from_port", "to", "to_port"];
const GROUP_FIELDS: &[&str] = &["name", "inputs", "outputs", "nodes", "edges"];
const GROUP_PORT_FIELDS: &[&str] = &["name", "type_refs", "node", "port"];

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

/// A mapping whose keys must all be among the allowed fields; an unknown
/// key is an error named at the key's own path. All three structural
/// mappings — the document, a node, an edge — walk their keys this way.
fn fields<'v>(value: &'v Value, path: &str, allowed: &[&str]) -> Result<&'v Mapping, LoadError> {
    let fields = as_mapping(value, path)?;
    for key in fields.keys() {
        if !key.as_str().is_some_and(|text| allowed.contains(&text)) {
            let text = key_text(key);
            return Err(unknown_field(&format!("{path}.{text}"), &text, allowed));
        }
    }
    Ok(fields)
}

fn load_document(root: &Value) -> Result<GraphDefinition, LoadError> {
    let fields = fields(root, DOCUMENT, DOCUMENT_FIELDS)?;
    let schema_version = check_schema_version(
        fields
            .get("schema_version")
            .ok_or_else(|| err(DOCUMENT, "missing field `schema_version`"))?,
    )?;
    let name = match fields.get("name") {
        Some(value) => Some(as_str(value, "name")?.to_owned()),
        None => None,
    };
    let nodes = load_nodes(
        fields
            .get(NODES)
            .ok_or_else(|| err(DOCUMENT, "missing field `nodes`"))?,
        NODES,
    )?;
    let edges = load_edges(
        fields
            .get(EDGES)
            .ok_or_else(|| err(DOCUMENT, "missing field `edges`"))?,
        EDGES,
    )?;
    // The version gates the shape: groups arrived with version 2, so a
    // version 1 document carrying them is refused, naming the version it
    // carries — never a v1 file silently gaining meaning.
    let groups = match fields.get(GROUPS) {
        Some(value) => {
            if schema_version == 1 {
                return Err(err(
                    "schema_version",
                    format!(
                        "a version 1 document carries a `groups` section; groups arrived with version {SCHEMA_VERSION}"
                    ),
                ));
            }
            load_groups(value)?
        }
        None => Vec::new(),
    };
    cross_check("", "the file", &nodes, &edges)?;
    // Whatever versions were read, what a definition is is written as: the
    // current shape, so a dump of a loaded version 1 file upgrades it.
    Ok(GraphDefinition {
        schema_version: SCHEMA_VERSION,
        name,
        nodes,
        edges,
        groups,
    })
}

fn check_schema_version(value: &Value) -> Result<u64, LoadError> {
    let version = value.as_u64().ok_or_else(|| {
        let not = match value {
            Value::Number(number) => format!(", not {number}"),
            other => format!(", not {}", kind(other)),
        };
        err(
            "schema_version",
            format!("the schema version must be a non-negative integer{not}"),
        )
    })?;
    if version == 0 || version > SCHEMA_VERSION {
        return Err(err(
            "schema_version",
            format!(
                "unsupported schema version {version}; this reader understands versions 1 to {SCHEMA_VERSION}"
            ),
        ));
    }
    Ok(version)
}

fn load_nodes(value: &Value, path: &str) -> Result<Vec<NodeInstance>, LoadError> {
    let items = as_sequence(value, path)?;
    items
        .iter()
        .enumerate()
        .map(|(index, node)| load_node(node, &format!("{path}[{index}]")))
        .collect()
}

fn load_node(node: &Value, path: &str) -> Result<NodeInstance, LoadError> {
    let fields = fields(node, path, NODE_FIELDS)?;
    let uuid = load_uuid(
        fields
            .get("uuid")
            .ok_or_else(|| err(path, "missing field `uuid`"))?,
        &format!("{path}.uuid"),
    )?;
    let type_ref = as_str(
        fields
            .get("type_ref")
            .ok_or_else(|| err(path, "missing field `type_ref`"))?,
        &format!("{path}.type_ref"),
    )?
    .to_owned();
    let label = match fields.get("label") {
        Some(value) => Some(as_str(value, &format!("{path}.label"))?.to_owned()),
        None => None,
    };
    let parameters = match fields.get("parameters") {
        Some(value) => load_parameters(value, &format!("{path}.parameters"))?,
        None => BTreeMap::new(),
    };
    let metadata = match fields.get("metadata") {
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
            read_parameter(value).map_err(|message| err(format!("{path}.{name}"), message))?,
        );
    }
    Ok(parameters)
}

/// The parameter value one plain scalar carries — the loader's one
/// reading, shared with the editor's commit path, so a committed value
/// and a hand-written one are the same value. The refusal is the bare
/// problem; callers name where the value sat.
fn read_parameter(value: &Value) -> Result<ParameterValue, String> {
    match value {
        Value::Bool(value) => Ok(ParameterValue::Bool(*value)),
        Value::Number(number) => {
            if let Some(int) = number.as_i64() {
                Ok(ParameterValue::Int(int))
            } else if number.is_f64() {
                Ok(ParameterValue::Float(number.as_f64().unwrap()))
            } else {
                Err(format!(
                    "the integer {number} does not fit a signed 64-bit integer"
                ))
            }
        }
        Value::String(value) => Ok(ParameterValue::Str(value.clone())),
        other => Err(format!(
            "must be a plain scalar — boolean, integer, float, or string — not {}",
            kind(other)
        )),
    }
}

/// The parameter value the typed text reads as, parsed exactly as the
/// same text hand-written into a file is parsed: one plain YAML scalar,
/// boolean, integer, float, or string. The commit crosses the seam as
/// raw text for this reason — the reading lives here, once.
pub(crate) fn read_parameter_text(text: &str) -> Result<ParameterValue, String> {
    let value =
        serde_yaml::from_str::<Value>(text).map_err(|_| "is not a plain scalar".to_owned())?;
    read_parameter(&value)
}

fn load_edges(value: &Value, path: &str) -> Result<Vec<Edge>, LoadError> {
    let items = as_sequence(value, path)?;
    items
        .iter()
        .enumerate()
        .map(|(index, edge)| load_edge(edge, &format!("{path}[{index}]")))
        .collect()
}

fn load_edge(edge: &Value, path: &str) -> Result<Edge, LoadError> {
    let fields = fields(edge, path, EDGE_FIELDS)?;
    Ok(Edge {
        from: load_uuid(
            fields
                .get("from")
                .ok_or_else(|| err(path, "missing field `from`"))?,
            &format!("{path}.from"),
        )?,
        from_port: as_str(
            fields
                .get("from_port")
                .ok_or_else(|| err(path, "missing field `from_port`"))?,
            &format!("{path}.from_port"),
        )?
        .to_owned(),
        to: load_uuid(
            fields
                .get("to")
                .ok_or_else(|| err(path, "missing field `to`"))?,
            &format!("{path}.to"),
        )?,
        to_port: as_str(
            fields
                .get("to_port")
                .ok_or_else(|| err(path, "missing field `to_port`"))?,
            &format!("{path}.to_port"),
        )?
        .to_owned(),
    })
}

/// A document's groups: each loaded, each name unique in the document.
fn load_groups(value: &Value) -> Result<Vec<GroupDefinition>, LoadError> {
    let items = as_sequence(value, GROUPS)?;
    let mut groups = Vec::with_capacity(items.len());
    let mut names = HashSet::new();
    for (index, group) in items.iter().enumerate() {
        let group = load_group(group, index)?;
        if !names.insert(group.name.clone()) {
            return Err(err(
                format!("{GROUPS}[{index}]"),
                format!("duplicate group name `{}`", group.name),
            ));
        }
        groups.push(group);
    }
    Ok(groups)
}

fn load_group(group: &Value, index: usize) -> Result<GroupDefinition, LoadError> {
    let path = format!("{GROUPS}[{index}]");
    let fields = fields(group, &path, GROUP_FIELDS)?;
    let name = as_str(
        fields
            .get("name")
            .ok_or_else(|| err(&path, "missing field `name`"))?,
        &format!("{path}.name"),
    )?
    .to_owned();
    let inputs = load_group_ports(
        fields
            .get("inputs")
            .ok_or_else(|| err(&path, "missing field `inputs`"))?,
        &format!("{path}.inputs"),
    )?;
    let outputs = load_group_ports(
        fields
            .get("outputs")
            .ok_or_else(|| err(&path, "missing field `outputs`"))?,
        &format!("{path}.outputs"),
    )?;
    let nodes = load_nodes(
        fields
            .get(NODES)
            .ok_or_else(|| err(&path, "missing field `nodes`"))?,
        &format!("{path}.{NODES}"),
    )?;
    let edges = load_edges(
        fields
            .get(EDGES)
            .ok_or_else(|| err(&path, "missing field `edges`"))?,
        &format!("{path}.{EDGES}"),
    )?;
    // The group's graph is closed, and every binding lands inside it: both
    // ends of an inner edge and both bindings of an exposed port name one
    // of the group's own nodes.
    let contained = nodes.iter().map(|node| node.uuid).collect::<HashSet<_>>();
    for (field, side, ports) in [
        ("inputs", "input", &inputs),
        ("outputs", "output", &outputs),
    ] {
        for (index, port) in ports.iter().enumerate() {
            if !contained.contains(&port.node) {
                return Err(err(
                    format!("{path}.{field}[{index}]"),
                    format!(
                        "the exposed {side} port `{}` binds node {}, which the group does not contain",
                        port.name, port.node
                    ),
                ));
            }
        }
    }
    cross_check(
        &format!("{path}."),
        &format!("group `{name}`"),
        &nodes,
        &edges,
    )?;
    Ok(GroupDefinition {
        name,
        inputs,
        outputs,
        nodes,
        edges,
    })
}

fn load_group_ports(value: &Value, path: &str) -> Result<Vec<GroupPort>, LoadError> {
    let items = as_sequence(value, path)?;
    let mut ports = Vec::with_capacity(items.len());
    let mut names = HashSet::new();
    for (index, port) in items.iter().enumerate() {
        let port = load_group_port(port, &format!("{path}[{index}]"))?;
        if !names.insert(port.name.clone()) {
            return Err(err(
                format!("{path}[{index}]"),
                format!("duplicate port name `{}`", port.name),
            ));
        }
        ports.push(port);
    }
    Ok(ports)
}

fn load_group_port(value: &Value, path: &str) -> Result<GroupPort, LoadError> {
    let fields = fields(value, path, GROUP_PORT_FIELDS)?;
    let type_refs = as_sequence(
        fields
            .get("type_refs")
            .ok_or_else(|| err(path, "missing field `type_refs`"))?,
        &format!("{path}.type_refs"),
    )?
    .iter()
    .enumerate()
    .map(|(index, reference)| {
        as_str(reference, &format!("{path}.type_refs[{index}]")).map(str::to_owned)
    })
    .collect::<Result<Vec<_>, _>>()?;
    Ok(GroupPort {
        name: as_str(
            fields
                .get("name")
                .ok_or_else(|| err(path, "missing field `name`"))?,
            &format!("{path}.name"),
        )?
        .to_owned(),
        type_refs,
        node: load_uuid(
            fields
                .get("node")
                .ok_or_else(|| err(path, "missing field `node`"))?,
            &format!("{path}.node"),
        )?,
        port: as_str(
            fields
                .get("port")
                .ok_or_else(|| err(path, "missing field `port`"))?,
            &format!("{path}.port"),
        )?
        .to_owned(),
    })
}

/// The cross-checks between a graph's node list and its edge list — the
/// top-level's, and each group's: uuids unique within the graph, edges
/// landing on nodes of the same graph, at most one upstream per input, an
/// input not carrying both a parameter value and a connection. `prefix`
/// locates the nodes and edges in the document, and `scope` names the
/// graph an error speaks of.
fn cross_check(
    prefix: &str,
    scope: &str,
    nodes: &[NodeInstance],
    edges: &[Edge],
) -> Result<(), LoadError> {
    let mut indexes = HashMap::<Uuid, usize>::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Some(first) = indexes.insert(node.uuid, index) {
            return Err(err(
                format!("{prefix}{NODES}[{index}]"),
                format!(
                    "duplicate node uuid {}; already used at {prefix}{NODES}[{first}]",
                    node.uuid
                ),
            ));
        }
    }
    let mut connected = HashMap::<(Uuid, &str), usize>::new();
    for (index, edge) in edges.iter().enumerate() {
        let path = format!("{prefix}{EDGES}[{index}]");
        for (uuid, end) in [(&edge.from, "from"), (&edge.to, "to")] {
            if !indexes.contains_key(uuid) {
                return Err(err(
                    &path,
                    format!("the edge's `{end}` node {uuid} is not defined in {scope}"),
                ));
            }
        }
        // The edge's nodes are known to be in this graph by here, so
        // each is named the way the file names it — the loader consults
        // no registry, so an unlabelled node reads as its type reference.
        let landed = &nodes[indexes[&edge.to]];
        if let Some(first) = connected.insert((edge.to, edge.to_port.as_str()), index) {
            return Err(err(
                &path,
                format!(
                    "input `{}` of node {} receives more than one connection; {prefix}{EDGES}[{first}] already feeds it",
                    edge.to_port,
                    landed.name(None)
                ),
            ));
        }
        if landed.parameters.contains_key(&edge.to_port) {
            return Err(err(
                &path,
                format!(
                    "input `{}` of node {} holds a parameter value and receives a connection; an input carries one or the other",
                    edge.to_port,
                    landed.name(None)
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
