//! The compiler: the pure transformation from a graph definition — the
//! parsed shape of a graph file, or whatever the editor holds mid-edit — to
//! a compiled graph, the immutable object an engine runs.
//!
//! [`compile`] takes the definition and the collected [`Registry`] and
//! returns what it found: the compiled graph when the definition is
//! correct, every compile error when it is not — so one recompile shows
//! all that still stands between the user and a runnable graph — and
//! beside either the non-fatal warnings. Every problem, error or warning,
//! carries the node instances it names, so the editor marks its canvas,
//! the failed-start report, and a headless printout all read one
//! structure and none of them parses the messages to find where a
//! problem lives. It is pure: no I/O, no global state, and the same
//! definition and registry always yield the same result. The registry is
//! passed in rather than read from the process, so compiling never rescans
//! registrations and recompiling an edited graph is a cheap, wholly
//! in-memory operation — the stop–edit–recompile–restart loop is a
//! first-class path, and recomputing the problems after every edit rides
//! it.
//!
//! Validation is what correctness requires and nothing more. The graph must
//! be a DAG — a cycle is an error naming the cycle — every edge must land on
//! ports that exist on its node types, and an input takes at most one
//! upstream connection while an output may fan out freely. A node instance
//! carries a unique uuid, a parameter must name an input port or one of the
//! type's declared choices, and an input carries a parameter or a
//! connection — never both. A declared choice must hold one of its options
//! on every instance: absent or outside the set is a compile error, so a
//! behaviour reads its setting knowing compile guaranteed it, and nothing
//! connects to a choice or waits on it. An input left with
//! neither a connection nor a parameter value is *not* of itself an error:
//! it compiles unconnected, and whether that is a fault only the node type
//! can say — one whose behaviour gates on the input declares it through
//! `check_parameters`. What compile does flag is the hang gate: an input
//! neither connected nor parameterised while another of the node's inputs
//! is connected or parameterised compiles clean and then never fires,
//! hanging its run until stopped — a trap otherwise discoverable only by
//! spending a run, so it is compile's first non-fatal warning, advisory in
//! every consumer, naming the node, the input, and the consequence. The
//! warning set grows only when another warning earns its place.
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
//! A definition may also carry groups — node types the document itself
//! defines, a nested graph packaged with exposed ports ([`crate::graph`]).
//! Compiling flattens them before anything else: every group instance is
//! replaced by its group's inner graph under derived identities
//! ([`inner_identity`]), boundary edges rewired through the exposed ports'
//! bindings, an exposed input's parameter literal fed inside — so the
//! engine runs one flat graph with no knowledge that groups exist, and
//! every inner-node event and error names the group instance and inner
//! node it belongs to by the composed label and the derived identity.
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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use uuid::Uuid;

mod flatten;
mod literal;

pub use flatten::inner_identity;

use crate::graph::{Edge, GraphDefinition, NodeInstance, ParameterValue};
use crate::registry::Registry;
use crate::value::Value;
use crate::{Choice, Conversion, DataType, NodeType, Port};

/// Compile a graph definition into a runnable graph, or report every compile
/// error found, with the non-fatal warnings beside either.
///
/// The definition's groups are flattened out first (see [`inner_identity`]
/// for the identities inner nodes take), and everything below runs on the
/// flat graph — the only shape the compiler, the engine, and their events
/// have ever known.
pub fn compile(definition: &GraphDefinition, registry: &Registry) -> CompileResult {
    let flat = flatten::flatten(definition, registry);
    let mut result = compile_flat(
        &flat.nodes,
        &flat.edges,
        definition.name.clone(),
        flat.errors,
        registry,
    );
    // A problem naming an inner node is re-pointed at the group instance
    // whose collapsed node holds it, so a mark lands where the canvas can
    // show it; the message keeps the composed name of the inner node.
    for problem in result.errors.iter_mut().chain(result.warnings.iter_mut()) {
        problem.nodes = problem
            .nodes
            .iter()
            .map(|uuid| flat.provenance.get(uuid).copied().unwrap_or(*uuid))
            .collect();
    }
    result
}

/// The compile of an already-flat node and edge list: every check, type
/// resolution, and family resolution the definition must pass, and the
/// compiled graph when it all holds. `errors` arrives preloaded with the
/// flattening's own.
fn compile_flat(
    nodes: &[NodeInstance],
    edges: &[Edge],
    name: Option<String>,
    mut errors: Vec<Problem>,
    registry: &Registry,
) -> CompileResult {
    let mut warnings = Vec::new();

    let mut instances = Instances::new();
    for instance in nodes {
        let node_type = registry.node_type(&instance.type_ref);
        if node_type.is_none() {
            errors.push(error(
                format!(
                    "node {} instantiates `{}`, which no group in the document defines and no linked plugin declares",
                    instance.name(node_type),
                    instance.type_ref
                ),
                vec![instance.uuid],
            ));
        }
        match instances.entry(instance.uuid) {
            std::collections::btree_map::Entry::Occupied(first) => {
                // The complaint's subject is the shared identity. Two
                // nodes may read alike — two unlabelled counters both read
                // `counter` — and then the colliding uuid is the only key
                // the file offers to tell them apart, so the message names
                // it; where the names differ they point on their own.
                let (first_instance, first_type) = *first.get();
                let first_name = first_instance.name(first_type);
                let name = instance.name(node_type);
                let identity = if first_name == name {
                    format!(" {}", instance.uuid)
                } else {
                    String::new()
                };
                errors.push(error(
                    format!(
                        "the nodes `{first_name}` and `{name}` claim the same identity{identity}"
                    ),
                    vec![instance.uuid],
                ));
            }
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert((instance, node_type));
            }
        }
    }

    if let Some(cycle) = find_cycle(nodes, edges, &instances) {
        let named = cycle
            .iter()
            .map(|uuid| {
                let (instance, node_type) = instances[uuid];
                instance.name(node_type)
            })
            .collect::<Vec<_>>()
            .join(" → ");
        // A self-loop's walk names its node twice; the mark list says
        // where the problem lives, once is enough.
        let mut marks = cycle;
        marks.dedup();
        errors.push(error(format!("cycle: {named}"), marks));
    }

    let mut connections = Vec::with_capacity(edges.len());
    let mut fed_by = HashMap::<(Uuid, &'static str), Uuid>::new();
    // Where each input's feed comes from, for the family sources below:
    // the upstream instance, its port, and the resolved connection type.
    let mut feeds = HashMap::<(Uuid, &'static str), (Uuid, &'static str, &'static DataType)>::new();
    for edge in edges {
        let from = instances.get(&edge.from).copied();
        let to = instances.get(&edge.to).copied();
        for (uuid, role) in [(edge.from, "from"), (edge.to, "to")] {
            if !instances.contains_key(&uuid) {
                errors.push(error(
                    format!(
                        "edge {} `{}` → {} `{}`: the {role} node is not defined in the graph",
                        endpoint(from, edge.from),
                        edge.from_port,
                        endpoint(to, edge.to),
                        edge.to_port
                    ),
                    vec![uuid],
                ));
            }
        }

        let mut output = None;
        if let Some((instance, Some(node_type))) = from {
            output = port(node_type.outputs, &edge.from_port);
            if output.is_none() {
                errors.push(error(
                    format!(
                        "node {} has no output port `{}`",
                        instance.name(Some(node_type)),
                        edge.from_port
                    ),
                    vec![edge.from],
                ));
            }
        }
        let mut input = None;
        if let Some((instance, Some(node_type))) = to {
            input = port(node_type.inputs, &edge.to_port);
            if input.is_none() {
                errors.push(error(
                    format!(
                        "node {} has no input port `{}`",
                        instance.name(Some(node_type)),
                        edge.to_port
                    ),
                    vec![edge.to],
                ));
            }
        }
        if let Some(input) = input {
            if let Some(first) = fed_by.insert((edge.to, input.name), edge.from) {
                let (instance, node_type) = instances[&edge.to];
                errors.push(error(
                    format!(
                        "input `{}` of node {} receives more than one connection (from {} and {})",
                        input.name,
                        instance.name(node_type),
                        endpoint(instances.get(&first).copied(), first),
                        endpoint(from, edge.from)
                    ),
                    vec![edge.to],
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
                    errors.push(error(
                        format!(
                            "connection {} `{}` ({}) → {} `{}` ({}): no exact match and no declared conversion bridges them",
                            from_instance.name(from_node_type), edge.from_port, output.type_refs.join(", "),
                            to_instance.name(to_node_type), edge.to_port, input.type_refs.join(", ")
                        ),
                        vec![edge.from, edge.to],
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
                // A declared choice's name is a parameter key beside the
                // port-named ones: the setting rides the parameter map, so
                // it is read here, against the options the declaration
                // offers.
                match declared_choice(node_type, name) {
                    Some(choice) => match choice_parameter(choice, literal, registry) {
                        Ok(chosen) => {
                            parameters.insert(choice.name, chosen);
                        }
                        Err(message) => errors.push(error(
                            format!("node {}: {message}", instance.name(Some(node_type))),
                            vec![*uuid],
                        )),
                    },
                    None => errors.push(error(
                        format!(
                            "node {}: parameter `{}` names neither an input port nor a declared choice",
                            instance.name(Some(node_type)),
                            name
                        ),
                        vec![*uuid],
                    )),
                }
                continue;
            };
            if fed_by.contains_key(&(*uuid, input.name)) {
                errors.push(error(
                    format!(
                        "input `{}` of node {} holds a parameter value and receives a connection; an input carries one or the other",
                        input.name,
                        instance.name(Some(node_type))
                    ),
                    vec![*uuid],
                ));
                continue;
            }
            if input.family.is_some() {
                literals_of.entry(*uuid).or_default().push((input, literal));
            } else {
                match literal::resolve_literal(literal, input.type_refs, registry) {
                    Some((resolved_type, value)) => {
                        parameters.insert(input.name, CompiledParameter { resolved_type, value });
                    }
                    None => errors.push(error(
                        format!(
                            "node {}: input `{}`: literal {literal} does not match declared types {} — no exact match and no declared conversion bridges them",
                            instance.name(Some(node_type)),
                            input.name,
                            input.type_refs.join(", ")
                        ),
                        vec![*uuid],
                    )),
                }
            }
        }
        for choice in node_type.choices {
            if !instance.parameters.contains_key(choice.name) {
                errors.push(error(
                    format!(
                        "node {}: choice `{}` holds no value — it must be one of {}",
                        instance.name(Some(node_type)),
                        choice.name,
                        choice.options.join(", ")
                    ),
                    vec![*uuid],
                ));
            }
        }
        parameters_of.insert(*uuid, parameters);
    }

    // The hang gate: an input neither connected nor parameterised beside
    // one that is. The graph compiles — the model allows the node to wait
    // — but the starving input never arrives, the node never fires, and
    // the run hangs until stopped: undiscoverable except by spending a
    // run, so compile warns, naming the node, the input, and what will
    // happen. Advisory everywhere: it blocks nothing, here or in any
    // consumer, and a headless compile shows it the same way.
    for (uuid, (instance, node_type)) in &instances {
        let Some(node_type) = node_type else { continue };
        let starving: Vec<&str> = node_type
            .inputs
            .iter()
            .filter(|input| {
                !fed_by.contains_key(&(*uuid, input.name))
                    && !instance.parameters.contains_key(input.name)
            })
            .map(|input| input.name)
            .collect();
        if starving.is_empty() || starving.len() == node_type.inputs.len() {
            continue;
        }
        for name in starving {
            warnings.push(Problem {
                message: format!(
                    "node {}: input `{name}` is neither connected nor parameterised — the node will never fire, hanging the run until it is stopped",
                    instance.name(Some(node_type))
                ),
                nodes: vec![*uuid],
            });
        }
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
                errors.push(error(
                    format!(
                        "node {}: the ports of its `{}` family declare different member sets ({}); a family resolves across one shared set",
                        instance.name(Some(node_type)),
                        name,
                        ports.iter().map(|port| port.type_refs.join(", ")).collect::<Vec<_>>().join(" / ")
                    ),
                    vec![*uuid],
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
                    errors.push(error(
                        format!(
                            "node {}: the ports of its `{}` family ({}) carry no connection and no parameter value — the family cannot resolve to a type",
                            instance.name(node_type),
                            job.name,
                            job.ports.iter().map(|port| format!("`{}`", port.name)).collect::<Vec<_>>().join(", ")
                        ),
                        vec![job.node],
                    ));
                }
                FamilyResolution::Conflict => {
                    failed.insert(key);
                    let (instance, node_type) = instances[&job.node];
                    errors.push(error(
                        format!(
                            "node {}: the ports of its `{}` family cannot resolve to one type — {}",
                            instance.name(node_type),
                            job.name,
                            job.sources
                                .iter()
                                .map(|source| source_describes(source, &instances))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        vec![job.node],
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
            match literal::resolve_literal(literal, &[member.name], registry) {
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
                    errors.push(error(
                        format!(
                            "node {}: input `{}`: literal {literal} does not fit the `{}` family's resolved type {}",
                            instance.name(node_type),
                            input.name,
                            input.family.expect("a family port"),
                            member.name
                        ),
                        vec![*uuid],
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
                errors.push(error(
                    format!(
                        "connection {} `{}` → {} `{}`: the resolved family type cannot reach the other side",
                        from_instance.name(from_node_type),
                        connection.from_port,
                        to_instance.name(to_node_type),
                        connection.to_port
                    ),
                    vec![connection.from, connection.to],
                ));
            }
        }
    }

    let mut nodes = BTreeMap::new();
    let mut fed_of = BTreeMap::<Uuid, BTreeSet<&'static str>>::new();
    for (uuid, input) in fed_by.keys() {
        fed_of.entry(*uuid).or_default().insert(*input);
    }
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
            fed: fed_of.remove(uuid).unwrap_or_default(),
        };
        if let Some(check) = node_type.check_parameters {
            for message in check(&compiled) {
                errors.push(error(
                    format!("node {}: {message}", instance.name(Some(node_type))),
                    vec![*uuid],
                ));
            }
        }
        nodes.insert(*uuid, compiled);
    }

    if errors.is_empty() {
        CompileResult {
            graph: Some(CompiledGraph {
                name,
                nodes,
                connections,
            }),
            errors,
            warnings,
        }
    } else {
        CompileResult {
            graph: None,
            errors,
            warnings,
        }
    }
}

/// What a compile found: the compiled graph when the definition is
/// correct, every compile error when it is not, and the non-fatal
/// warnings beside either.
#[derive(Clone, Debug)]
pub struct CompileResult {
    /// The compiled graph — present only when `errors` is empty. A
    /// definition carrying warnings but no errors compiles as freely as a
    /// clean one: warnings advise, they never refuse.
    pub graph: Option<CompiledGraph>,
    /// Every compile error found, each naming the node instances it
    /// speaks of.
    pub errors: Vec<Problem>,
    /// The non-fatal warnings: advisory in every consumer, changing no
    /// outcome here or anywhere else.
    pub warnings: Vec<Problem>,
}

/// One problem a compile found: the message as the compiler writes it,
/// and the node instances the problem names. The one structure every
/// view of the problem reads — the editor's canvas marks, the
/// failed-start report, a headless printout — so none of them finds
/// where a problem lives by parsing its message.
#[derive(Clone, Debug)]
pub struct Problem {
    pub message: String,
    /// The node instances the problem names, in the order the message
    /// names them. A problem whose location no node carries — an edge
    /// naming an instance the graph never defined — lists that instance
    /// all the same; a mark simply has nothing to land on.
    pub nodes: Vec<Uuid>,
}

fn error(message: String, nodes: Vec<Uuid>) -> Problem {
    Problem { message, nodes }
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
    /// resolved — and already converted where a declared conversion bridged
    /// — and beside them the option each declared choice holds, by the
    /// choice's name ([`CompiledNode::choice`] reads one).
    pub parameters: BTreeMap<&'static str, CompiledParameter>,
    /// The concrete type each of the instance's port families resolved to,
    /// by family name. A behaviour built over a family-declared node reads
    /// values as the member its family resolved to.
    pub families: BTreeMap<&'static str, &'static DataType>,
    /// The input ports the graph's wiring feeds, by port name — the rest of
    /// an input's carriage is a parameter value, or nothing at all.
    pub fed: BTreeSet<&'static str>,
}

impl CompiledNode {
    /// The option one of the type's declared choices holds on this
    /// instance. Compile refuses an instance whose choice is absent or
    /// outside its options, so a behaviour reads its setting here without a
    /// fallback to invent.
    pub fn choice(&self, name: &str) -> &str {
        self.parameters
            .get(name)
            .and_then(|chosen| chosen.value.get::<String>())
            .map(String::as_str)
            .expect("compile refuses an instance whose declared choice holds no option")
    }
}

/// A parameter value that compiled: the data type the literal resolved to,
/// and the runtime value the engine feeds that input as a stream that yields
/// once and completes. A declared choice's value rides the same shape: the
/// chosen option as text.
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

fn declared_choice(node_type: &NodeType, name: &str) -> Option<&'static Choice> {
    node_type.choices.iter().find(|choice| choice.name == name)
}

/// One declared choice's compiled value: the option the instance named,
/// carried as text — the shape every choice's value takes. Anything the
/// declaration does not offer, a value that is not text among it, is the
/// message the caller reports against the options.
fn choice_parameter(
    choice: &Choice,
    literal: &ParameterValue,
    registry: &Registry,
) -> Result<CompiledParameter, String> {
    let chosen = match literal {
        ParameterValue::Str(text) if choice.options.contains(&text.as_str()) => text,
        _ => {
            return Err(format!(
                "choice `{}` holds {literal}, which is outside its options ({})",
                choice.name,
                choice.options.join(", ")
            ))
        }
    };
    Ok(CompiledParameter {
        resolved_type: registry
            .data_type_by_id(crate::scalars::STRING)
            .expect("core ships the String scalar every choice's value is carried as"),
        value: Value::new(crate::scalars::STRING, chosen.clone()),
    })
}

/// The graph's node instances by uuid, each with the node type it
/// instantiates where one is declared — what every message naming a node
/// reads from.
type Instances<'g> = BTreeMap<Uuid, (&'g NodeInstance, Option<&'static NodeType>)>;

/// How a message names one end of an edge: the node's own name where the
/// graph defines it, else the uuid the file itself writes for it — an
/// undefined node has no label anywhere, and that uuid is the only name
/// the user's own document gives it.
fn endpoint(known: Option<(&NodeInstance, Option<&'static NodeType>)>, uuid: Uuid) -> String {
    known.map_or_else(
        || uuid.to_string(),
        |(instance, node_type)| instance.name(node_type).to_owned(),
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
                literal::resolve_literal(literal, &[member], registry).is_some()
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
fn source_describes(source: &FamilySource<'_>, instances: &Instances<'_>) -> String {
    match source {
        FamilySource::Type(port, resolved_type) => {
            format!("`{port}` carries {}", resolved_type.name)
        }
        FamilySource::Deferred(port, uuid, family) => {
            format!(
                "`{port}` waits on node {}'s `{family}` family",
                endpoint(instances.get(uuid).copied(), *uuid)
            )
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

/// The first cycle the graph's edges form, as the uuids around it: the
/// shared walk ([`first_cycle`]) over the compiled instances' adjacency,
/// starting from each node in definition order and following edges in
/// edge order.
fn find_cycle(
    nodes: &[NodeInstance],
    edges: &[Edge],
    instances: &BTreeMap<Uuid, (&NodeInstance, Option<&'static NodeType>)>,
) -> Option<Vec<Uuid>> {
    let mut adjacency = HashMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        if instances.contains_key(&edge.from) && instances.contains_key(&edge.to) {
            adjacency.entry(edge.from).or_default().push(edge.to);
        }
    }
    let roots = nodes.iter().map(|node| node.uuid).collect::<Vec<_>>();
    first_cycle(&roots, &adjacency)
}

/// The first cycle a depth-first walk meets, as the keys around it, or
/// none. The walk starts from each root in the order given and follows
/// each key's neighbours in the order stored, so the same graph always
/// names the same cycle. The graph cycle and the group-reference cycle
/// walk this one function — two adjacency maps, one determinism contract.
fn first_cycle<K>(roots: &[K], adjacency: &HashMap<K, Vec<K>>) -> Option<Vec<K>>
where
    K: Copy + Eq + std::hash::Hash,
{
    let mut done = HashSet::new();
    let mut active = HashSet::new();
    let mut stack = Vec::<(K, usize)>::new();
    for root in roots {
        if done.contains(root) {
            continue;
        }
        active.insert(*root);
        stack.push((*root, 0));
        while let Some(&(current, visited)) = stack.last() {
            let neighbor = adjacency
                .get(&current)
                .and_then(|neighbors| neighbors.get(visited))
                .copied();
            match neighbor {
                Some(next) => {
                    stack.last_mut().expect("just read").1 += 1;
                    if active.contains(&next) {
                        let position = stack
                            .iter()
                            .position(|(candidate, _)| *candidate == next)
                            .expect("an active key is on the path");
                        let mut cycle: Vec<K> =
                            stack[position..].iter().map(|(key, _)| *key).collect();
                        cycle.push(next);
                        return Some(cycle);
                    }
                    if !done.contains(&next) {
                        active.insert(next);
                        stack.push((next, 0));
                    }
                }
                None => {
                    let (key, _) = stack.pop().expect("just read");
                    active.remove(&key);
                    done.insert(key);
                }
            }
        }
    }
    None
}
