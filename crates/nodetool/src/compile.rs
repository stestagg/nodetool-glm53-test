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
//!
//! One node type can declare a set of its ports as one *family* — the same
//! family name on several ports — whose members the ports spell out: the
//! family resolves to one concrete type per instance, drawn from the
//! members, under the same exact-or-declared-conversion rules every
//! connection already follows. An exact agreement among the connected
//! sources wins; otherwise the first declared member every source reaches
//! — connections by one declared conversion, literals by fitting — is the
//! resolution, walking the family's declaration order, never registry
//! order. Sources that agree on nothing, and a family with no source at
//! all, are compile errors naming the node, the ports, and their types.
//! The resolution is recorded on the compiled node, where the behaviour
//! builder stamps the instance's behaviour for the resolved type; a
//! connection into or out of a family port is rebuilt against the
//! resolution, so what the compiled graph carries is what will flow.

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
                node_name(instance, node_type),
                instance.type_ref
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
            .map(|uuid| {
                let (instance, node_type) = instances[uuid];
                node_name(instance, node_type)
            })
            .collect::<Vec<_>>()
            .join(" → ");
        errors.push(format!("cycle: {named}"));
    }

    let mut connections = Vec::with_capacity(definition.edges.len());
    let mut fed_by = HashMap::<(Uuid, &'static str), Uuid>::new();
    // Where each input's feed comes from, for the family sources below:
    // the upstream instance, its port, and the resolved connection type.
    let mut feeds = HashMap::<(Uuid, &'static str), (Uuid, &'static str, &'static DataType)>::new();
    for edge in &definition.edges {
        let from = instances.get(&edge.from).copied();
        let to = instances.get(&edge.to).copied();
        for (uuid, role) in [(edge.from, "from"), (edge.to, "to")] {
            if !instances.contains_key(&uuid) {
                errors.push(format!(
                    "edge {} `{}` → {} `{}`: the {role} node {uuid} is not defined in the graph",
                    edge.from, edge.from_port, edge.to, edge.to_port
                ));
            }
        }

        let mut output = None;
        if let Some((instance, Some(node_type))) = from {
            output = port(node_type.outputs, &edge.from_port);
            if output.is_none() {
                errors.push(format!(
                    "node {} has no output port `{}`",
                    node_name(instance, Some(node_type)),
                    edge.from_port
                ));
            }
        }
        let mut input = None;
        if let Some((instance, Some(node_type))) = to {
            input = port(node_type.inputs, &edge.to_port);
            if input.is_none() {
                errors.push(format!(
                    "node {} has no input port `{}`",
                    node_name(instance, Some(node_type)),
                    edge.to_port
                ));
            }
        }
        if let Some(input) = input {
            if let Some(first) = fed_by.insert((edge.to, input.name), edge.from) {
                let (instance, node_type) = instances[&edge.to];
                errors.push(format!(
                    "input `{}` of node {} receives more than one connection (from {first} and {})",
                    input.name,
                    node_name(instance, node_type),
                    edge.from
                ));
            }
        }

        if let (Some(output), Some(input)) = (output, input) {
            match resolve_types(output.type_refs, input.type_refs, registry) {
                Some((resolved_type, conversion)) => {
                    feeds.insert(
                        (edge.to, input.name),
                        (edge.from, output.name, resolved_type),
                    );
                    connections.push(Connection {
                        from: edge.from,
                        from_port: output.name,
                        to: edge.to,
                        to_port: input.name,
                        resolved_type,
                        conversion,
                    });
                }
                None => {
                    let (from_instance, from_node_type) = instances[&edge.from];
                    let (to_instance, to_node_type) = instances[&edge.to];
                    errors.push(format!(
                        "connection {} `{}` ({}) → {} `{}` ({}): no exact match and no declared conversion bridges them",
                        node_name(from_instance, from_node_type), edge.from_port, output.type_refs.join(", "),
                        node_name(to_instance, to_node_type), edge.to_port, input.type_refs.join(", ")
                    ));
                }
            }
        }
    }

    // Parameters resolve per node; a family port's literal defers until the
    // family resolved, since the member it must fit is the family's choice.
    let mut parameters_of = BTreeMap::<Uuid, BTreeMap<&'static str, CompiledParameter>>::new();
    let mut literals_of = BTreeMap::<Uuid, Vec<(&'static Port, &'static ParameterValue)>>::new();
    for (uuid, (instance, node_type)) in &instances {
        let Some(node_type) = node_type else { continue };
        let mut parameters = BTreeMap::new();
        for (name, literal) in &instance.parameters {
            let Some(input) = port(node_type.inputs, name) else {
                errors.push(format!(
                    "node {}: parameter `{}` does not name an input port",
                    node_name(instance, Some(node_type)),
                    name
                ));
                continue;
            };
            if fed_by.contains_key(&(*uuid, input.name)) {
                errors.push(format!(
                    "input `{}` of node {} holds a parameter value and receives a connection; an input carries one or the other",
                    input.name,
                    node_name(instance, Some(node_type))
                ));
                continue;
            }
            if input.family.is_some() {
                literals_of.entry(*uuid).or_default().push((input, literal));
            } else {
                match resolve_literal(literal, input.type_refs, registry) {
                    Some((resolved_type, value)) => {
                        parameters.insert(input.name, CompiledParameter { resolved_type, value });
                    }
                    None => errors.push(format!(
                        "node {}: input `{}`: literal {literal} does not match declared types {} — no exact match and no declared conversion bridges them",
                        node_name(instance, Some(node_type)),
                        input.name,
                        input.type_refs.join(", ")
                    )),
                }
            }
        }
        parameters_of.insert(*uuid, parameters);
    }

    // One job per port family on one node instance: the family's ports, the
    // member list they declare, and every source that constrains the
    // resolution — a connected type, a parameter literal, or a connection
    // from an upstream family output whose own resolution must come first.
    let mut jobs = Vec::<FamilyJob>::new();
    for (uuid, (instance, node_type)) in &instances {
        let Some(node_type) = node_type else { continue };
        let mut grouped = BTreeMap::<&'static str, Vec<&'static Port>>::new();
        for port in node_type.inputs.iter().chain(node_type.outputs.iter()) {
            if let Some(family) = port.family {
                grouped.entry(family).or_default().push(port);
            }
        }
        for (name, ports) in grouped {
            let members = ports[0].type_refs;
            if ports.iter().any(|port| port.type_refs != members) {
                errors.push(format!(
                    "node {}: the ports of its `{}` family declare different member sets ({}); a family resolves across one shared set",
                    node_name(instance, Some(node_type)),
                    name,
                    ports.iter().map(|port| port.type_refs.join(", ")).collect::<Vec<_>>().join(" / ")
                ));
                continue;
            }
            let mut sources = Vec::new();
            for input in ports.iter().filter(|port| {
                node_type
                    .inputs
                    .iter()
                    .any(|declared| declared.name == port.name)
            }) {
                if let Some((from, from_port, resolved_type)) = feeds.get(&(*uuid, input.name)) {
                    let upstream_family = instances
                        .get(from)
                        .and_then(|(_, node_type)| *node_type)
                        .and_then(|node_type| port(node_type.outputs, from_port))
                        .and_then(|output| output.family);
                    sources.push(match upstream_family {
                        Some(family) => FamilySource::Deferred(input.name, *from, family),
                        None => FamilySource::Type(input.name, resolved_type),
                    });
                } else if let Some(literal) = literals_of
                    .get(uuid)
                    .and_then(|literals| literals.iter().find(|(port, _)| port.name == input.name))
                {
                    sources.push(FamilySource::Literal(literal.0.name, literal.1));
                }
            }
            jobs.push(FamilyJob {
                node: *uuid,
                name,
                members,
                ports,
                sources,
            });
        }
    }

    // Resolve every family. A family fed by an upstream family waits for
    // that resolution; the loop runs until no job can make progress, which
    // on an acyclic graph leaves every family resolved or failed.
    let mut resolved = HashMap::<(Uuid, &'static str), &'static DataType>::new();
    let mut failed = HashSet::<(Uuid, &'static str)>::new();
    loop {
        let mut progress = false;
        for job in &mut jobs {
            let key = (job.node, job.name);
            if resolved.contains_key(&key) || failed.contains(&key) {
                continue;
            }
            let mut ready = true;
            for source in &mut job.sources {
                if let FamilySource::Deferred(port, upstream, family) = source {
                    if let Some(member) = resolved.get(&(*upstream, *family)) {
                        *source = FamilySource::Type(port, member);
                    } else if failed.contains(&(*upstream, *family)) {
                        // The upstream already reported its own failure:
                        // this family is its consequence, reported there.
                        failed.insert(key);
                        ready = false;
                        break;
                    } else {
                        ready = false;
                        break;
                    }
                }
            }
            if !ready {
                continue;
            }
            progress = true;
            match resolve_family(&job.sources, job.members, registry) {
                FamilyResolution::Resolved(member) => {
                    resolved.insert(key, member);
                }
                FamilyResolution::NoSource => {
                    failed.insert(key);
                    let (instance, node_type) = instances[&job.node];
                    errors.push(format!(
                        "node {}: the ports of its `{}` family ({}) carry no connection and no parameter value — the family cannot resolve to a type",
                        node_name(instance, node_type),
                        job.name,
                        job.ports.iter().map(|port| format!("`{}`", port.name)).collect::<Vec<_>>().join(", ")
                    ));
                }
                FamilyResolution::Conflict => {
                    failed.insert(key);
                    let (instance, node_type) = instances[&job.node];
                    errors.push(format!(
                        "node {}: the ports of its `{}` family cannot resolve to one type — {}",
                        node_name(instance, node_type),
                        job.name,
                        job.sources
                            .iter()
                            .map(source_describes)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
        }
        if !progress {
            break;
        }
    }

    // Fill the resolved families in, resolve each family literal against the
    // member it must now fit, and rebuild every connection that touches a
    // family port, so the compiled graph carries the types that will flow.
    let mut families_of = BTreeMap::<Uuid, BTreeMap<&'static str, &'static DataType>>::new();
    for ((uuid, family), member) in &resolved {
        families_of
            .entry(*uuid)
            .or_default()
            .insert(*family, *member);
    }
    for (uuid, literals) in &literals_of {
        for (input, literal) in literals {
            let Some(member) = resolved.get(&(*uuid, input.family.expect("a family port"))) else {
                continue;
            };
            match resolve_literal(literal, &[member.name], registry) {
                Some((resolved_type, value)) => {
                    parameters_of.entry(*uuid).or_default().insert(
                        input.name,
                        CompiledParameter {
                            resolved_type,
                            value,
                        },
                    );
                }
                None => {
                    let (instance, node_type) = instances[uuid];
                    errors.push(format!(
                        "node {}: input `{}`: literal {literal} does not fit the `{}` family's resolved type {}",
                        node_name(instance, node_type),
                        input.name,
                        input.family.expect("a family port"),
                        member.name
                    ));
                }
            }
        }
    }
    for connection in &mut connections {
        let Some(from_type) = instances
            .get(&connection.from)
            .and_then(|(_, node_type)| *node_type)
        else {
            continue;
        };
        let Some(to_type) = instances
            .get(&connection.to)
            .and_then(|(_, node_type)| *node_type)
        else {
            continue;
        };
        let from_family =
            port(from_type.outputs, connection.from_port).and_then(|output| output.family);
        let to_family = port(to_type.inputs, connection.to_port).and_then(|input| input.family);
        if from_family.is_none() && to_family.is_none() {
            continue;
        }
        let from_refs =
            match from_family.and_then(|family| resolved.get(&(connection.from, family))) {
                Some(member) => vec![member.name],
                None => port(from_type.outputs, connection.from_port)
                    .map(|output| output.type_refs.to_vec())
                    .unwrap_or_default(),
            };
        let to_refs = match to_family.and_then(|family| resolved.get(&(connection.to, family))) {
            Some(member) => vec![member.name],
            None => port(to_type.inputs, connection.to_port)
                .map(|input| input.type_refs.to_vec())
                .unwrap_or_default(),
        };
        match resolve_types(&from_refs, &to_refs, registry) {
            Some((resolved_type, conversion)) => {
                connection.resolved_type = resolved_type;
                connection.conversion = conversion;
            }
            None => {
                let (from_instance, from_node_type) = instances[&connection.from];
                let (to_instance, to_node_type) = instances[&connection.to];
                errors.push(format!(
                    "connection {} `{}` → {} `{}`: the resolved family type cannot reach the other side",
                    node_name(from_instance, from_node_type),
                    connection.from_port,
                    node_name(to_instance, to_node_type),
                    connection.to_port
                ));
            }
        }
    }

    let mut nodes = BTreeMap::new();
    for (uuid, (instance, node_type)) in &instances {
        let Some(node_type) = node_type else { continue };
        let compiled = CompiledNode {
            node_type,
            label: instance
                .label
                .clone()
                .unwrap_or_else(|| node_type.label.to_owned()),
            parameters: parameters_of.remove(uuid).unwrap_or_default(),
            families: families_of.remove(uuid).unwrap_or_default(),
        };
        if let Some(check) = node_type.check_parameters {
            for message in check(&compiled) {
                errors.push(format!(
                    "node {}: {message}",
                    node_name(instance, Some(node_type))
                ));
            }
        }
        nodes.insert(*uuid, compiled);
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
    /// The concrete type each of the instance's port families resolved to,
    /// by family name. A behaviour built over a family-declared node reads
    /// values as the member its family resolved to.
    pub families: BTreeMap<&'static str, &'static DataType>,
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

/// What a compile error names a node instance by: the label the run would
/// call it — the instance's override, else the type's default — with the
/// uuid beside it, the dialect the engine's own reports speak. A node
/// whose type no linked plugin declares falls back to the type reference,
/// the name the editor shows its placeholder by.
fn node_name(instance: &NodeInstance, node_type: Option<&NodeType>) -> String {
    format!(
        "{} ({})",
        instance
            .label
            .as_deref()
            .unwrap_or_else(|| node_type.map_or(instance.type_ref.as_str(), |t| t.label)),
        instance.uuid
    )
}

/// One port family waiting to resolve on one node instance.
struct FamilyJob<'g> {
    node: Uuid,
    name: &'static str,
    /// The members the family's ports declare, in declaration order — the
    /// order the deterministic tiebreak walks.
    members: &'static [&'static str],
    /// The family's ports, for error messages.
    ports: Vec<&'static Port>,
    /// What constrains the resolution: one source per fed family input.
    sources: Vec<FamilySource<'g>>,
}

/// One source a family's resolution must satisfy, fed into the family
/// input port the source names.
enum FamilySource<'g> {
    /// A connection whose type already resolved.
    Type(&'static str, &'static DataType),
    /// A connection from an upstream family output: the concrete type is
    /// that instance's resolution, known only once it resolves.
    Deferred(&'static str, Uuid, &'static str),
    /// A parameter literal: it pins no type, but the resolved member must
    /// fit it.
    Literal(&'static str, &'g ParameterValue),
}

/// What one family's sources agreed on.
enum FamilyResolution {
    /// The member every source reaches.
    Resolved(&'static DataType),
    /// No source at all: nothing connected, no literal.
    NoSource,
    /// Sources that cannot agree on one declared member.
    Conflict,
}

/// Resolve one family: an exact agreement wins — every source a connected
/// type, all the same declared member — otherwise the first declared member
/// every source reaches by the connection rules, walking the family's
/// declaration order. The walk never consults registry iteration order, so
/// resolution is deterministic.
fn resolve_family(
    sources: &[FamilySource<'_>],
    members: &[&'static str],
    registry: &Registry,
) -> FamilyResolution {
    if sources.is_empty() {
        return FamilyResolution::NoSource;
    }
    // Exact agreement: every source a connected type, all naming the same
    // declared member.
    let connected = sources
        .iter()
        .map(|source| match source {
            FamilySource::Type(_, resolved_type) => Some(*resolved_type),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    if let Some(resolved_types) = connected {
        let first = resolved_types[0];
        if resolved_types
            .iter()
            .all(|resolved_type| resolved_type.name == first.name)
            && members.contains(&first.name)
        {
            return FamilyResolution::Resolved(first);
        }
    }
    for &member in members {
        let Some(target) = registry.data_type(member) else {
            continue;
        };
        let unified = sources.iter().all(|source| match source {
            FamilySource::Type(_, resolved_type) => {
                resolved_type.name == member
                    || resolved_type
                        .conversions
                        .iter()
                        .any(|conversion| conversion.target == target.id)
            }
            FamilySource::Literal(_, literal) => {
                resolve_literal(literal, &[member], registry).is_some()
            }
            FamilySource::Deferred(..) => false,
        });
        if unified {
            return FamilyResolution::Resolved(target);
        }
    }
    FamilyResolution::Conflict
}

/// What one source says about itself, for the conflict error naming the
/// ports and their types.
fn source_describes(source: &FamilySource<'_>) -> String {
    match source {
        FamilySource::Type(port, resolved_type) => {
            format!("`{port}` carries {}", resolved_type.name)
        }
        FamilySource::Deferred(port, uuid, family) => {
            format!("`{port}` waits on node {uuid}'s `{family}` family")
        }
        FamilySource::Literal(port, literal) => {
            format!("`{port}` holds the literal {literal:?}")
        }
    }
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
