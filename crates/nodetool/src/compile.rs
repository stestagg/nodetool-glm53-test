//! The compiler: the pure transformation from a graph definition — the
//! parsed shape of a graph file, or whatever the editor holds mid-edit — to
//! a compiled graph, the immutable object an engine runs.
//!
//! [`compile`] takes the definition and the collected [`Registry`] and
//! returns either the compiled graph or every compile error it could find,
//! so one recompile shows all that still stands between the user and a
//! runnable graph. It is pure: no I/O, no global state, and the same
//! definition and registry always yield the same result. The registry is
//! passed in rather than read from the process, so compiling never rescans
//! registrations and recompiling an edited graph is a cheap, wholly
//! in-memory operation — the stop–edit–recompile–restart loop is a
//! first-class path.
//!
//! Validation is what correctness requires and nothing more. The graph must
//! be a DAG — a cycle is an error naming the cycle — every edge must land on
//! ports that exist on its node types, and an input takes at most one
//! upstream connection while an output may fan out freely. A node instance
//! carries a unique uuid, a parameter must name an input port, and an input
//! carries a parameter or a connection — never both. An input left with
//! neither a connection nor a parameter value is *not* an error: it
//! compiles unconnected; flagging such inputs early, if wanted, belongs to
//! the editor's warn-early set, not to compilation.
//!
//! Every connection's type is resolved before the graph is built, across the
//! unions its ports declare: the first exact match, walking the upstream's
//! declared order, wins; failing that, the first declared conversion that
//! fits. The tiebreak walks declared order, never registry iteration order,
//! so resolution is deterministic. A conversion rides the connection: the
//! compiled connection carries the registry's conversion function and the
//! engine applies it as values flow. Nothing is written back into the
//! definition — a saved file stays exactly as the user wrote it, and the
//! conversion is visible only on the compile result, where an editor can
//! badge it.
//!
//! Parameter literals ride the same resolution, the literal's YAML scalar
//! kind (integer, float, boolean, string) standing in as the upstream side
//! against the input port's declared union. The kinds stand in as the base
//! scalar set: an integer literal matches an integer-declared member by
//! kind, and bridges further only through a declared conversion from one
//! of those scalars — so `3` into an `f64`-only port resolves through the
//! declared `i32`→`f64` conversion and the compiled parameter already holds
//! the converted value. Whichever member a literal resolves to, the value
//! must fit it: `300` into an `i8`-declared input is an error, not a
//! parameter the engine misreads. A literal the declared types neither
//! match nor bridge is a compile error.
//!
//! The compiled graph addresses nodes by instance uuid alone. Nothing in
//! its shape branches on what a node is called or which type it is, and a
//! node type backed by a subgraph — its compiled body a nested compiled
//! graph — slots into [`CompiledNode`] without reworking the shape.

use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

use crate::graph::{Edge, GraphDefinition, NodeInstance, ParameterValue};
use crate::registry::Registry;
use crate::value::Value;
use crate::{Conversion, ConvertFn, DataType, NodeType, Port};

/// Compile a graph definition into a runnable graph, or report every compile
/// error found.
pub fn compile(
    definition: &GraphDefinition,
    registry: &Registry,
) -> Result<CompiledGraph, Vec<String>> {
    let mut errors = Vec::new();

    let mut instances = BTreeMap::<Uuid, (&NodeInstance, Option<&'static NodeType>)>::new();
    for instance in &definition.nodes {
        let node_type = registry.node_type(&instance.type_ref);
        if node_type.is_none() {
            errors.push(format!(
                "node {} instantiates `{}`, which no linked plugin declares",
                instance.uuid, instance.type_ref
            ));
        }
        match instances.entry(instance.uuid) {
            std::collections::btree_map::Entry::Occupied(_) => {
                errors.push(format!("duplicate node uuid {}", instance.uuid));
            }
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert((instance, node_type));
            }
        }
    }

    if let Some(cycle) = find_cycle(&definition.nodes, &definition.edges, &instances) {
        let named = cycle
            .iter()
            .map(|uuid| format!("{uuid} (`{}`)", instances[uuid].0.type_ref))
            .collect::<Vec<_>>()
            .join(" → ");
        errors.push(format!("cycle: {named}"));
    }

    let mut connections = Vec::with_capacity(definition.edges.len());
    let mut fed_by = HashMap::<(Uuid, &'static str), Uuid>::new();
    for edge in &definition.edges {
        let from = instances.get(&edge.from);
        let to = instances.get(&edge.to);
        for (uuid, role) in [(edge.from, "from"), (edge.to, "to")] {
            if !instances.contains_key(&uuid) {
                errors.push(format!(
                    "edge {} `{}` → {} `{}`: the {role} node {uuid} is not defined in the graph",
                    edge.from, edge.from_port, edge.to, edge.to_port
                ));
            }
        }
        let from_type = from.and_then(|(_, node_type)| *node_type);
        let to_type = to.and_then(|(_, node_type)| *node_type);

        let mut output = None;
        if let Some(node_type) = from_type {
            output = port(node_type.outputs, &edge.from_port);
            if output.is_none() {
                errors.push(format!(
                    "node {} (`{}`) has no output port `{}`",
                    edge.from, node_type.type_ref, edge.from_port
                ));
            }
        }
        let mut input = None;
        if let Some(node_type) = to_type {
            input = port(node_type.inputs, &edge.to_port);
            if input.is_none() {
                errors.push(format!(
                    "node {} (`{}`) has no input port `{}`",
                    edge.to, node_type.type_ref, edge.to_port
                ));
            }
        }
        if let Some(input) = input {
            if let Some(first) = fed_by.insert((edge.to, input.name), edge.from) {
                errors.push(format!(
                    "input `{}` of node {} receives more than one connection (from {} and {})",
                    input.name, edge.to, first, edge.from
                ));
            }
        }

        if let (Some(output), Some(input)) = (output, input) {
            match resolve_types(output.type_refs, input.type_refs, registry) {
                Some((resolved_type, conversion)) => connections.push(Connection {
                    from: edge.from,
                    from_port: output.name,
                    to: edge.to,
                    to_port: input.name,
                    resolved_type,
                    conversion,
                }),
                None => errors.push(format!(
                    "connection {} `{}` ({}) → {} `{}` ({}): no exact match and no declared conversion bridges them",
                    edge.from, edge.from_port, output.type_refs.join(", "), edge.to, edge.to_port, input.type_refs.join(", ")
                )),
            }
        }
    }

    let mut nodes = BTreeMap::new();
    for (uuid, (instance, node_type)) in &instances {
        let Some(node_type) = node_type else { continue };
        let mut parameters = BTreeMap::new();
        for (name, literal) in &instance.parameters {
            let Some(input) = port(node_type.inputs, name) else {
                errors.push(format!(
                    "node {} (`{}`): parameter `{}` does not name an input port",
                    uuid, node_type.type_ref, name
                ));
                continue;
            };
            if fed_by.contains_key(&(*uuid, input.name)) {
                errors.push(format!(
                    "input `{}` of node {} holds a parameter value and receives a connection; an input carries one or the other",
                    input.name, uuid
                ));
                continue;
            }
            match resolve_literal(literal, input.type_refs, registry) {
                Some((resolved_type, value)) => {
                    parameters.insert(input.name, CompiledParameter { resolved_type, value });
                }
                None => errors.push(format!(
                    "node {} (`{}`): input `{}`: literal {literal} does not match declared types {} — no exact match and no declared conversion bridges them",
                    uuid, node_type.type_ref, input.name, input.type_refs.join(", ")
                )),
            }
        }
        nodes.insert(
            *uuid,
            CompiledNode {
                node_type,
                label: instance
                    .label
                    .clone()
                    .unwrap_or_else(|| node_type.label.to_owned()),
                parameters,
            },
        );
    }

    if errors.is_empty() {
        Ok(CompiledGraph {
            name: definition.name.clone(),
            nodes,
            connections,
        })
    } else {
        Err(errors)
    }
}

/// A compiled graph: immutable once produced, addressed by instance uuid,
/// ready for an engine to run.
#[derive(Clone, Debug)]
pub struct CompiledGraph {
    /// The definition's name, if it gave one.
    pub name: Option<String>,
    /// The graph's nodes, by instance uuid.
    pub nodes: BTreeMap<Uuid, CompiledNode>,
    /// The connections, in the definition's edge order.
    pub connections: Vec<Connection>,
}

/// One compiled node instance.
#[derive(Clone, Debug)]
pub struct CompiledNode {
    /// The node type this instance instantiates.
    pub node_type: &'static NodeType,
    /// What a run names the node by: the label the definition gave the
    /// instance, or the type's default label when it gave none.
    pub label: String,
    /// Values fixed for input ports as literals, by port name, each type
    /// resolved — and already converted where a declared conversion bridged.
    pub parameters: BTreeMap<&'static str, CompiledParameter>,
}

/// A parameter value that compiled: the data type the literal resolved to,
/// and the runtime value the engine feeds that input as a stream that yields
/// once and completes.
#[derive(Clone, Debug)]
pub struct CompiledParameter {
    pub resolved_type: &'static DataType,
    pub value: Value,
}

/// One compiled connection: a definition edge whose type resolved before the
/// graph was built. Fan-out — one output feeding many inputs — is preserved.
#[derive(Clone, Debug)]
pub struct Connection {
    pub from: Uuid,
    pub from_port: &'static str,
    pub to: Uuid,
    pub to_port: &'static str,
    /// The connection's type, resolved from the ports' declared unions.
    pub resolved_type: &'static DataType,
    /// The declared conversion the connection rides, if any: the engine
    /// applies it as values flow.
    pub conversion: Option<Conversion>,
}

fn port<'p>(ports: &'p [Port], name: &str) -> Option<&'p Port> {
    ports.iter().find(|port| port.name == name)
}

/// Resolve a connection's type across the two ports' declared unions, in
/// declaration order: the first exact match wins; failing that, the first
/// declared conversion that fits. Registry iteration order is never
/// consulted.
fn resolve_types(
    from: &[&str],
    to: &[&str],
    registry: &Registry,
) -> Option<(&'static DataType, Option<Conversion>)> {
    for &name in from {
        if let Some(data_type) = registry.data_type(name) {
            if to.contains(&name) {
                return Some((data_type, None));
            }
        }
    }
    for &name in from {
        let Some(data_type) = registry.data_type(name) else {
            continue;
        };
        for &conversion in data_type.conversions {
            if let Some(target) = registry.data_type_by_id(conversion.target) {
                if to.contains(&target.name) {
                    return Some((target, Some(conversion)));
                }
            }
        }
    }
    None
}

/// Resolve a parameter literal against an input port's declared union, under
/// the connection rules: the literal's YAML scalar kind stands in as the
/// upstream side. Returns the resolved data type and the runtime value the
/// engine feeds — converted here, at compile time, where a declared
/// conversion bridged.
fn resolve_literal(
    literal: &ParameterValue,
    declared: &[&str],
    registry: &Registry,
) -> Option<(&'static DataType, Value)> {
    let kind = literal_kind(literal);
    for &name in declared {
        let Some(data_type) = registry.data_type(name) else {
            continue;
        };
        if scalar_kind(data_type.name) == Some(kind) {
            if let Some(value) = materialize(literal, data_type) {
                return Some((data_type, value));
            }
        }
    }
    for &name in declared {
        let Some(data_type) = registry.data_type(name) else {
            continue;
        };
        for source in kind_names(kind) {
            let Some(source_type) = registry.data_type(source) else {
                continue;
            };
            for conversion in source_type.conversions {
                if conversion.target == data_type.id {
                    if let Some(value) = convert_literal(literal, source_type, conversion.convert) {
                        return Some((data_type, value));
                    }
                }
            }
        }
    }
    None
}

/// The four YAML scalar kinds a parameter literal can carry, and the scalar
/// names standing in for each kind: the literal's kind is its "upstream
/// side" under the connection rules, and the lists fix the order conversion
/// candidates are tried in.
const KIND_NAMES: &[(ScalarKind, &[&str])] = &[
    (
        ScalarKind::Integer,
        &["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"],
    ),
    (ScalarKind::Float, &["f32", "f64"]),
    (ScalarKind::Bool, &["bool"]),
    (ScalarKind::Str, &["String"]),
];

#[derive(Clone, Copy, PartialEq)]
enum ScalarKind {
    Integer,
    Float,
    Bool,
    Str,
}

fn literal_kind(literal: &ParameterValue) -> ScalarKind {
    match literal {
        ParameterValue::Bool(_) => ScalarKind::Bool,
        ParameterValue::Int(_) => ScalarKind::Integer,
        ParameterValue::Float(_) => ScalarKind::Float,
        ParameterValue::Str(_) => ScalarKind::Str,
    }
}

fn scalar_kind(name: &str) -> Option<ScalarKind> {
    KIND_NAMES
        .iter()
        .find(|(_, names)| names.contains(&name))
        .map(|(kind, _)| *kind)
}

fn kind_names(kind: ScalarKind) -> &'static [&'static str] {
    KIND_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == kind)
        .map(|(_, names)| *names)
        .expect("every ScalarKind is listed in KIND_NAMES")
}

/// Present a literal as a runtime value of the data type `source` names, so
/// exact matches and declared conversion functions read a concrete scalar.
/// An integer outside the source's range yields None: the candidate simply
/// does not bridge. A float wider than f32's range yields None too; one that
/// fits is narrowed.
fn materialize(literal: &ParameterValue, source: &DataType) -> Option<Value> {
    match (literal, source.name) {
        (ParameterValue::Bool(value), "bool") => Some(Value::new(source.id, *value)),
        (ParameterValue::Int(value), "i64") => Some(Value::new(source.id, *value)),
        (ParameterValue::Int(value), "i32") => {
            Some(Value::new(source.id, i32::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "i16") => {
            Some(Value::new(source.id, i16::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "i8") => {
            Some(Value::new(source.id, i8::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u64") => {
            Some(Value::new(source.id, u64::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u32") => {
            Some(Value::new(source.id, u32::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u16") => {
            Some(Value::new(source.id, u16::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u8") => {
            Some(Value::new(source.id, u8::try_from(*value).ok()?))
        }
        (ParameterValue::Float(value), "f64") => Some(Value::new(source.id, *value)),
        (ParameterValue::Float(value), "f32") => {
            let narrowed = *value as f32;
            if !narrowed.is_finite() {
                return None;
            }
            Some(Value::new(source.id, narrowed))
        }
        (ParameterValue::Str(value), "String") => Some(Value::new(source.id, value.clone())),
        _ => None,
    }
}

/// Apply a declared conversion to a literal, through the runtime value both
/// sides are written against.
fn convert_literal(
    literal: &ParameterValue,
    source: &DataType,
    convert: ConvertFn,
) -> Option<Value> {
    convert(&materialize(literal, source)?)
}

/// The first cycle a depth-first walk meets, as the uuids around it. The
/// walk starts from each node in definition order and follows edges in edge
/// order, so the same definition always names the same cycle.
fn find_cycle(
    nodes: &[NodeInstance],
    edges: &[Edge],
    instances: &BTreeMap<Uuid, (&NodeInstance, Option<&'static NodeType>)>,
) -> Option<Vec<Uuid>> {
    let mut adjacency = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        if instances.contains_key(&edge.from) && instances.contains_key(&edge.to) {
            adjacency.entry(edge.from).or_default().push(edge.to);
        }
    }
    let mut done = HashSet::new();
    let mut active = HashSet::new();
    let mut stack = Vec::<(Uuid, usize)>::new();
    for node in nodes {
        if done.contains(&node.uuid) {
            continue;
        }
        active.insert(node.uuid);
        stack.push((node.uuid, 0));
        while let Some(&(current, visited)) = stack.last() {
            let neighbor = adjacency
                .get(&current)
                .and_then(|neighbors| neighbors.get(visited))
                .copied();
            match neighbor {
                Some(uuid) => {
                    stack.last_mut().expect("just read").1 += 1;
                    if active.contains(&uuid) {
                        let position = stack
                            .iter()
                            .position(|(candidate, _)| *candidate == uuid)
                            .expect("an active node is on the path");
                        let mut cycle: Vec<Uuid> =
                            stack[position..].iter().map(|(uuid, _)| *uuid).collect();
                        cycle.push(uuid);
                        return Some(cycle);
                    }
                    if !done.contains(&uuid) {
                        active.insert(uuid);
                        stack.push((uuid, 0));
                    }
                }
                None => {
                    let (uuid, _) = stack.pop().expect("just read");
                    active.remove(&uuid);
                    done.insert(uuid);
                }
            }
        }
    }
    None
}
