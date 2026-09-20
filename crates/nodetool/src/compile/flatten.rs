//! Flattening: the compile-time pass that makes groups real. Every group
//! instance a definition carries is replaced by its group's inner graph —
//! inner nodes under identities derived from the chain of instances
//! enclosing them, boundary edges rewired through the exposed ports'
//! bindings, an exposed input's parameter literal fed to the port it
//! binds — so what the compiler's validation and the engine's run see is
//! one flat graph, the only shape either has ever known. The pass is pure
//! in-memory recursion, and the engine learns nothing of it.
//!
//! Only the groups the top level reaches have their declared interface
//! checked and their instances expanded — a group no instance references
//! is dead weight, the hand-writer's to see, not a failure mode to invent.
//! The interface check requires every binding to name an inner port that
//! exists and the exposed port's declared types to bridge to the port it
//! binds — the declared interface honest against the inside that backs
//! it. The group-reference cycle is the one check that runs wherever a
//! group sits, and the one that stops the flattening: expanding through a
//! cycle has no base case. Everything else an inner node violates — a
//! cycle through the boundary, a bad literal, a starving input — the
//! ordinary compile reports on the flattened graph, where it is one more
//! node or edge of the same kind it always judged.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::{error, first_cycle, resolve_types, Problem};
use crate::graph::{
    Edge, GraphDefinition, GroupDefinition, GroupPort, NodeInstance, ParameterValue,
};
use crate::registry::Registry;

/// The compiled identity of a node inside a group: derived deterministically
/// from the chain of group instances enclosing it — innermost first — plus
/// the node's own uuid, so the same inner uuid under different enclosing
/// chains derives distinct identities, and which group instance and inner
/// node a compiled identity names is recoverable by recomputing this fold.
/// Each enclosing instance is XORed in and the accumulator rotated one
/// bit, so chains that differ as paths derive identities that differ in
/// turn — the fold is order-sensitive, not a set.
///
/// Mirrored client-side (`ui/src/groups.js`) to decode run events:
/// change the two together.
pub fn inner_identity(chain: &[Uuid], node: Uuid) -> Uuid {
    let mut id = node.as_u128();
    for &instance in chain {
        id = (id ^ instance.as_u128()).rotate_left(1);
    }
    Uuid::from_u128(id)
}

/// A definition with its groups flattened out: every node left names a
/// node type, every edge lands on two of the nodes, and every inner
/// identity carries the provenance a canvas needs — the collapsed group
/// instance it sits inside.
pub(super) struct Flat {
    pub(super) nodes: Vec<NodeInstance>,
    pub(super) edges: Vec<Edge>,
    /// The compiled identity of an inner node → the group instance uuid
    /// whose collapsed node holds it.
    pub(super) provenance: HashMap<Uuid, Uuid>,
    pub(super) errors: Vec<Problem>,
}

/// Which side of a boundary an edge end crosses or a binding stands on:
/// an edge's `to` lands on an exposed input, its `from` leaves an exposed
/// output.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Input,
    Output,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Side::Input => "input",
            Side::Output => "output",
        }
    }
}

pub(super) fn flatten(definition: &GraphDefinition, registry: &Registry) -> Flat {
    let mut groups = HashMap::<&str, &GroupDefinition>::new();
    let mut errors = Vec::new();
    for group in &definition.groups {
        if groups.insert(group.name.as_str(), group).is_some() {
            // The loader refuses a duplicate; a definition built in memory
            // meets the same refusal here.
            errors.push(error(
                format!("duplicate group name `{}`", group.name),
                vec![],
            ));
        }
    }
    let mut flat = Flattener {
        groups,
        registry,
        flat: Flat {
            nodes: Vec::new(),
            edges: Vec::new(),
            provenance: HashMap::new(),
            errors,
        },
    };
    // A group no instance references is dead weight: its faults are the
    // hand-writer's to see, not a failure mode to invent, so the
    // collision and interface checks run on the groups the top level
    // reaches and nothing else. The cycle is the one check that keeps its
    // anywhere rule, and the one that stops the flattening — expanding
    // through a cycle has no base case.
    let live = instantiated(definition, &flat.groups);
    flat.check_collisions(definition, &live);
    if let Some(cycle) = group_cycle(definition, &flat.groups) {
        let doomed = doomed_instances(definition, &cycle);
        flat.flat
            .errors
            .push(error(format!("group cycle: {}", cycle.join(" → ")), doomed));
        return flat.flat;
    }
    flat.check_interfaces(definition, &live);
    flat.expand(definition);
    flat.rewire(definition);
    flat.flat
}

struct Flattener<'a> {
    groups: HashMap<&'a str, &'a GroupDefinition>,
    registry: &'a Registry,
    flat: Flat,
}

impl Flattener<'_> {
    /// A group whose name a linked plugin also claims would make its
    /// instances' type references ambiguous — the document's groups win
    /// the resolution, so the collision is an error, not a shadowing.
    /// Dead groups take no check: nothing runs them.
    fn check_collisions(&mut self, definition: &GraphDefinition, live: &HashSet<&str>) {
        for group in &definition.groups {
            if !live.contains(group.name.as_str()) || self.registry.node_type(&group.name).is_none()
            {
                continue;
            }
            let instances = definition
                .nodes
                .iter()
                .filter(|node| node.type_ref == group.name)
                .map(|node| node.uuid)
                .collect();
            self.flat.errors.push(error(
                format!(
                    "group `{}` collides with a node type a linked plugin declares; the group's instances would be ambiguous",
                    group.name
                ),
                instances,
            ));
        }
    }

    /// The interface each group the top level reaches declares, checked
    /// before anything is expanded: every binding names an inner node the
    /// group contains, the port it binds exists — an inner node's own
    /// port, or the exposed port of the nested group that node
    /// instantiates — and the exposed port's declared types can bridge to
    /// that port's, resolving against each other exactly as any edge does.
    fn check_interfaces(&mut self, definition: &GraphDefinition, live: &HashSet<&str>) {
        for group in &definition.groups {
            if !live.contains(group.name.as_str()) {
                continue;
            }
            let instances = definition
                .nodes
                .iter()
                .filter(|node| node.type_ref == group.name)
                .map(|node| node.uuid)
                .collect::<Vec<_>>();
            for (side, ports) in [(Side::Input, &group.inputs), (Side::Output, &group.outputs)] {
                for binding in ports {
                    let Some(refs) = self.bound_refs(group, binding, side, &instances) else {
                        continue;
                    };
                    let exposed = binding
                        .type_refs
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    let refs = refs.iter().map(String::as_str).collect::<Vec<_>>();
                    let bridged = match side {
                        Side::Input => resolve_types(&exposed, &refs, self.registry),
                        Side::Output => resolve_types(&refs, &exposed, self.registry),
                    };
                    if bridged.is_none() {
                        self.flat.errors.push(error(
                            format!(
                                "group `{}`: the exposed {} port `{}` declares {}, which cannot bridge to the port it binds ({})",
                                group.name,
                                side.name(),
                                binding.name,
                                binding.type_refs.join(", "),
                                refs.join(", ")
                            ),
                            instances.clone(),
                        ));
                    }
                }
            }
        }
    }

    /// The declared types of the port one binding binds, or none when the
    /// binding cannot be resolved — its own error reported here, or the
    /// node's unknown-type error reported where the node compiles.
    fn bound_refs(
        &mut self,
        group: &GroupDefinition,
        binding: &GroupPort,
        side: Side,
        instances: &[Uuid],
    ) -> Option<Vec<String>> {
        let Some(inner) = group.nodes.iter().find(|node| node.uuid == binding.node) else {
            self.flat.errors.push(error(
                format!(
                    "group `{}`: the exposed {} port `{}` binds node {}, which the group does not contain",
                    group.name,
                    side.name(),
                    binding.name,
                    binding.node
                ),
                instances.to_owned(),
            ));
            return None;
        };
        match self.groups.get(inner.type_ref.as_str()) {
            Some(nested) => {
                let ports = match side {
                    Side::Input => &nested.inputs,
                    Side::Output => &nested.outputs,
                };
                match ports.iter().find(|port| port.name == binding.port) {
                    Some(port) => Some(port.type_refs.clone()),
                    None => {
                        let label = inner.label.as_deref().unwrap_or(&nested.name);
                        self.flat.errors.push(error(
                            format!(
                                "group `{}`: the exposed {} port `{}` binds `{}` on the instance {label}, which does not expose it",
                                group.name,
                                side.name(),
                                binding.name,
                                binding.port,
                            ),
                            instances.to_owned(),
                        ));
                        None
                    }
                }
            }
            None => {
                let node_type = self.registry.node_type(&inner.type_ref)?;
                let ports = match side {
                    Side::Input => &node_type.inputs,
                    Side::Output => &node_type.outputs,
                };
                match ports.iter().find(|port| port.name == binding.port) {
                    Some(port) => {
                        Some(port.type_refs.iter().map(|name| name.to_string()).collect())
                    }
                    None => {
                        let label = inner.label.as_deref().unwrap_or(node_type.label);
                        self.flat.errors.push(error(
                            format!(
                                "group `{}`: the exposed {} port `{}` binds port `{}` on node {label}, which does not declare it",
                                group.name,
                                side.name(),
                                binding.name,
                                binding.port,
                            ),
                            instances.to_owned(),
                        ));
                        None
                    }
                }
            }
        }
    }

    /// Replace every group instance on the top level with its group's
    /// inner graph, recursing into the nested instances.
    fn expand(&mut self, definition: &GraphDefinition) {
        for instance in &definition.nodes {
            match self.groups.get(instance.type_ref.as_str()) {
                Some(group) => {
                    self.expand_group(instance, group, &[instance.uuid], instance.uuid, &[])
                }
                None => self.flat.nodes.push(instance.clone()),
            }
        }
    }

    /// One group instance's inside: its plain inner nodes under derived
    /// identities, labelled by the instance they sit in, its nested
    /// instances expanded in turn, the literals its exposed inputs carry
    /// routed through the bindings, and its inner edges rewritten across
    /// its own boundary.
    fn expand_group(
        &mut self,
        instance: &NodeInstance,
        group: &GroupDefinition,
        chain: &[Uuid],
        root: Uuid,
        literals: &[(String, ParameterValue)],
    ) {
        let label = instance.label.clone().unwrap_or_else(|| group.name.clone());
        // A parameter naming no exposed input would vanish through the
        // boundary unheard — the flat graph's "does not name an input
        // port" mistake, reported here where the boundary would hide it.
        for name in instance.parameters.keys() {
            if !group.inputs.iter().any(|binding| binding.name == *name) {
                self.flat.errors.push(error(
                    format!(
                        "group instance {label}: parameter `{name}` does not name an exposed input"
                    ),
                    vec![root],
                ));
            }
        }
        // The literals the instance's exposed inputs carry — its own, or
        // the ones an enclosing instance passed through this very
        // boundary — routed to the inner port each binding names: fed to
        // the flat node once it is pushed, or handed down as the nested
        // instance's inherited literal.
        let mut inherited = HashMap::<Uuid, Vec<(String, ParameterValue)>>::new();
        let mut plain = Vec::<(Uuid, String, &ParameterValue)>::new();
        for binding in &group.inputs {
            let own = instance.parameters.get(&binding.name);
            let passed = literals
                .iter()
                .find(|(name, _)| name == &binding.name)
                .map(|(_, value)| value);
            let literal = match (own, passed) {
                (None, None) => continue,
                (Some(_), Some(_)) => {
                    self.flat.errors.push(error(
                        format!(
                            "group instance {label} receives the exposed input `{}` through the boundary while carrying a parameter for it; an input carries one or the other",
                            binding.name
                        ),
                        vec![root],
                    ));
                    continue;
                }
                (Some(literal), _) | (None, Some(literal)) => literal,
            };
            let Some(inner) = group.nodes.iter().find(|node| node.uuid == binding.node) else {
                continue;
            };
            if self.groups.contains_key(inner.type_ref.as_str()) {
                inherited
                    .entry(binding.node)
                    .or_default()
                    .push((binding.port.clone(), literal.clone()));
            } else {
                plain.push((
                    inner_identity(chain, binding.node),
                    binding.port.clone(),
                    literal,
                ));
            }
        }
        for inner in &group.nodes {
            let identity = inner_identity(chain, inner.uuid);
            self.flat.provenance.insert(identity, root);
            match self.groups.get(inner.type_ref.as_str()) {
                Some(nested) => {
                    let mut nested_chain = chain.to_vec();
                    nested_chain.push(inner.uuid);
                    let empty = Vec::new();
                    let passed = inherited.get(&inner.uuid).unwrap_or(&empty);
                    self.expand_group(inner, nested, &nested_chain, root, passed);
                }
                None => {
                    let node_type = self.registry.node_type(&inner.type_ref);
                    let default = node_type.map_or(inner.type_ref.as_str(), |t| t.label);
                    self.flat.nodes.push(NodeInstance {
                        uuid: identity,
                        type_ref: inner.type_ref.clone(),
                        label: Some(format!(
                            "{label} · {}",
                            inner.label.as_deref().unwrap_or(default)
                        )),
                        parameters: inner.parameters.clone(),
                        metadata: Default::default(),
                    });
                }
            }
        }
        for (identity, port, literal) in plain {
            let Some(node) = self
                .flat
                .nodes
                .iter_mut()
                .find(|node| node.uuid == identity)
            else {
                continue;
            };
            if node
                .parameters
                .insert(port.clone(), literal.clone())
                .is_some()
            {
                let label = node.label.clone().unwrap_or_default();
                self.flat.errors.push(error(
                    format!(
                        "input `{port}` of {label} ({identity}) receives more than one parameter value; an input carries one"
                    ),
                    vec![root],
                ));
            }
        }
        for edge in &group.edges {
            let from = self.cross(group, chain, edge.from, &edge.from_port, Side::Output);
            let to = self.cross(group, chain, edge.to, &edge.to_port, Side::Input);
            if let (Some(from), Some(to)) = (from, to) {
                self.flat.edges.push(Edge {
                    from: from.0,
                    from_port: from.1,
                    to: to.0,
                    to_port: to.1,
                });
            }
        }
    }

    /// One end of an inner edge, resolved across this group's boundary: a
    /// plain node's derived identity and its port, or the inner port an
    /// exposed binding stands in for on a nested instance.
    fn cross(
        &mut self,
        group: &GroupDefinition,
        chain: &[Uuid],
        uuid: Uuid,
        port: &str,
        side: Side,
    ) -> Option<(Uuid, String)> {
        let Some(inner) = group.nodes.iter().find(|node| node.uuid == uuid) else {
            self.flat.errors.push(error(
                format!(
                    "an inner edge of group `{}` lands on node {}, which the group does not contain",
                    group.name, uuid
                ),
                vec![],
            ));
            return None;
        };
        match self.groups.get(inner.type_ref.as_str()) {
            Some(nested) => {
                // The nested instance joins the chain its inside derives
                // from — the same push the expansion and expose's own
                // recursion make — so an inner edge reaching through it
                // points at the identity the expansion actually pushed.
                let mut nested_chain = chain.to_vec();
                nested_chain.push(uuid);
                self.expose(nested, inner, &nested_chain, uuid, port, side)
            }
            None => Some((inner_identity(chain, uuid), port.to_owned())),
        }
    }

    /// The exposed port `port` of the group instance `instance`, followed
    /// to the inner port it binds — through the nested instances the
    /// binding chain may cross.
    fn expose(
        &mut self,
        group: &GroupDefinition,
        instance: &NodeInstance,
        chain: &[Uuid],
        instance_uuid: Uuid,
        port: &str,
        side: Side,
    ) -> Option<(Uuid, String)> {
        let ports = match side {
            Side::Input => &group.inputs,
            Side::Output => &group.outputs,
        };
        let Some(binding) = ports.iter().find(|exposed| exposed.name == port) else {
            let label = instance.label.as_deref().unwrap_or(&group.name);
            self.flat.errors.push(error(
                format!(
                    "group instance {label} has no exposed {} port `{port}`",
                    side.name()
                ),
                vec![instance_uuid],
            ));
            return None;
        };
        let Some(inner) = group.nodes.iter().find(|node| node.uuid == binding.node) else {
            // The interface checks reported the binding; nothing to rewire.
            return None;
        };
        match self.groups.get(inner.type_ref.as_str()) {
            Some(nested) => {
                let mut nested_chain = chain.to_vec();
                nested_chain.push(binding.node);
                self.expose(
                    nested,
                    inner,
                    &nested_chain,
                    binding.node,
                    &binding.port,
                    side,
                )
            }
            None => Some((inner_identity(chain, binding.node), binding.port.clone())),
        }
    }

    /// The top-level edges, each end resolved across a group boundary when
    /// it lands on a group instance's exposed port.
    fn rewire(&mut self, definition: &GraphDefinition) {
        let top = definition
            .nodes
            .iter()
            .map(|node| (node.uuid, node))
            .collect::<HashMap<_, _>>();
        for edge in &definition.edges {
            let from = self.end(&top, edge.from, &edge.from_port, Side::Output);
            let to = self.end(&top, edge.to, &edge.to_port, Side::Input);
            if let (Some(from), Some(to)) = (from, to) {
                self.flat.edges.push(Edge {
                    from: from.0,
                    from_port: from.1,
                    to: to.0,
                    to_port: to.1,
                });
            }
        }
    }

    fn end(
        &mut self,
        top: &HashMap<Uuid, &NodeInstance>,
        uuid: Uuid,
        port: &str,
        side: Side,
    ) -> Option<(Uuid, String)> {
        match top.get(&uuid) {
            Some(instance) => match self.groups.get(instance.type_ref.as_str()) {
                Some(group) => self.expose(group, instance, &[uuid], uuid, port, side),
                None => Some((uuid, port.to_owned())),
            },
            // A dangling end: the ordinary compile reports it, naming the
            // instance the definition carries.
            None => Some((uuid, port.to_owned())),
        }
    }
}

/// The groups transitively instantiated from the top level, through the
/// groups instantiating groups: the ones whose declared interface the
/// compile judges and whose names it defends against the registry.
fn instantiated<'a>(
    definition: &'a GraphDefinition,
    groups: &HashMap<&'a str, &'a GroupDefinition>,
) -> HashSet<&'a str> {
    let mut seen = HashSet::new();
    let mut queue: Vec<(&'a str, &'a GroupDefinition)> = definition
        .nodes
        .iter()
        .filter_map(|node| groups.get_key_value(node.type_ref.as_str()))
        .map(|(name, group)| (*name, *group))
        .collect();
    while let Some((name, group)) = queue.pop() {
        if !seen.insert(name) {
            continue;
        }
        for node in &group.nodes {
            if let Some((inner, nested)) = groups.get_key_value(node.type_ref.as_str()) {
                queue.push((inner, nested));
            }
        }
    }
    seen
}

/// The instances a group-reference cycle dooms: every top-level instance
/// whose group is a cycle member, or instantiates one directly or through
/// others — the error's marks landing on the canvas nodes the user sees,
/// where instances exist at all.
fn doomed_instances(definition: &GraphDefinition, cycle: &[String]) -> Vec<Uuid> {
    let mut doomed: HashSet<&str> = cycle.iter().map(String::as_str).collect();
    let mut grew = true;
    while grew {
        grew = false;
        for group in &definition.groups {
            if doomed.contains(group.name.as_str()) {
                continue;
            }
            if group
                .nodes
                .iter()
                .any(|node| doomed.contains(node.type_ref.as_str()))
            {
                doomed.insert(group.name.as_str());
                grew = true;
            }
        }
    }
    definition
        .nodes
        .iter()
        .filter(|node| doomed.contains(node.type_ref.as_str()))
        .map(|node| node.uuid)
        .collect()
}

/// The first group-reference cycle, as the group names around it: the
/// shared walk ([`super::first_cycle`]) over the groups' reference
/// adjacency, starting from each group in document order and following
/// references in node order. A cycle is an error wherever it sits:
/// nothing instantiating a member of one can ever compile.
fn group_cycle(
    definition: &GraphDefinition,
    groups: &HashMap<&str, &GroupDefinition>,
) -> Option<Vec<String>> {
    let adjacency = definition
        .groups
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group
                    .nodes
                    .iter()
                    .filter_map(|node| groups.get_key_value(node.type_ref.as_str()))
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let roots = definition
        .groups
        .iter()
        .map(|group| group.name.as_str())
        .collect::<Vec<_>>();
    first_cycle(&roots, &adjacency).map(|cycle| cycle.into_iter().map(str::to_owned).collect())
}
