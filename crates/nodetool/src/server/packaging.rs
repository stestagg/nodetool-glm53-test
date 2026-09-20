//! The two group gestures the editing catalogue carries: packaging a
//! selection into a group and unpacking a group instance back into its
//! nodes. Both are derivations from the definition the server holds — the
//! browser names the selection and, for packaging, the group, and
//! everything else (the exposed ports, the body, the positions) follows
//! from what is already there, so the browser never assembles the groups
//! format by hand. Both validate first and mutate once, so a refusal
//! leaves the definition exactly as it was and an applied gesture is one
//! atomic edit the whole definition push carries.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::graph::{
    Edge, GraphDefinition, GroupDefinition, GroupPort, Mapping, NodeInstance, Value,
};
use crate::registry::Registry;

/// Package the selection into a new group named `name`, replacing it on
/// the canvas with one collapsed instance that answers with its uuid.
///
/// The selection's nodes and the edges wholly inside it become the group's
/// body; each edge crossing the selection boundary becomes an exposed port
/// binding the inner port it stood in for — an output feeding the outside
/// becomes an exposed output serving all its external downstreams, an
/// edge from outside becomes an exposed input. Each port takes the bound
/// inner port's name, deduped across the new port set, and declares that
/// inner port's type references, resolved against the document's groups
/// before the linked types. The packaged nodes' stored positions move into
/// the body as offsets relative to the selection's centroid, a node
/// carrying no stored position contributing neither centroid nor offset,
/// and the instance sits at that centroid.
pub fn package(
    graph: &mut GraphDefinition,
    selection: &[Uuid],
    name: &str,
    registry: &Registry,
) -> Result<Uuid, String> {
    let mut selected: HashSet<Uuid> = HashSet::new();
    if selection.is_empty() {
        return Err("the selection is empty: there is nothing to package".to_owned());
    }
    for uuid in selection {
        if !graph.nodes.iter().any(|node| node.uuid == *uuid) {
            return Err("no such node in the definition".to_owned());
        }
        if !selected.insert(*uuid) {
            return Err(format!("node {uuid} is named twice in the selection"));
        }
    }
    if name.is_empty() {
        return Err("the group's name is empty".to_owned());
    }
    if graph.groups.iter().any(|group| group.name == name) {
        return Err(format!(
            "a group named `{name}` already exists in this document"
        ));
    }
    if registry.node_type(name).is_some() {
        return Err(format!(
            "the name `{name}` collides with a node type a linked plugin declares"
        ));
    }
    if let Some(who) = boundary_placeholder(graph, &selected, registry) {
        return Err(format!(
            "the selection's boundary crosses node {who}, whose type no group in the document \
             defines and no linked plugin declares; an exposed port would have no types to declare"
        ));
    }

    let centroid = centroid(graph, &selected);
    let body: Vec<NodeInstance> = graph
        .nodes
        .iter()
        .filter(|node| selected.contains(&node.uuid))
        .map(|node| {
            let mut packaged = node.clone();
            if let Some((x, y)) = stored_position(node) {
                if let Some((cx, cy)) = centroid {
                    set_position(&mut packaged.metadata, x - cx, y - cy);
                }
            }
            packaged
        })
        .collect();
    let body_edges: Vec<Edge> = graph
        .edges
        .iter()
        .filter(|edge| selected.contains(&edge.from) && selected.contains(&edge.to))
        .cloned()
        .collect();

    let instance_uuid = Uuid::new_v4();
    let (inputs, outputs, reattached) = exposed_ports(graph, registry, &selected, instance_uuid);
    let instance = NodeInstance {
        uuid: instance_uuid,
        type_ref: name.to_owned(),
        label: None,
        parameters: Default::default(),
        metadata: match centroid {
            Some((x, y)) => position_metadata(x, y),
            None => Default::default(),
        },
    };
    graph.nodes.retain(|node| !selected.contains(&node.uuid));
    graph
        .edges
        .retain(|edge| !selected.contains(&edge.from) && !selected.contains(&edge.to));
    graph.edges.extend(reattached);
    graph.groups.push(GroupDefinition {
        name: name.to_owned(),
        inputs,
        outputs,
        nodes: body,
        edges: body_edges,
    });
    graph.nodes.push(instance);
    Ok(instance_uuid)
}

/// Unpack the group instance `uuid` back into its nodes, answering the
/// uuids the nodes return under.
///
/// Each body node reappears at its stored offset from where the instance
/// sat; the external wires re-attach through the exposed ports' bindings,
/// a wire at a port name no binding declares having nothing to land on
/// and going with the instance; a value the instance held on an
/// unconnected exposed input lands on the bound inner input as its
/// parameter. The gesture always succeeds structurally: a returning uuid
/// that collides with a surviving node is reassigned fresh, and the group
/// definition leaves with the instance when no other instance references it.
pub fn unpack(
    graph: &mut GraphDefinition,
    uuid: Uuid,
    registry: &Registry,
) -> Result<Vec<Uuid>, String> {
    let at = graph
        .nodes
        .iter()
        .position(|node| node.uuid == uuid)
        .ok_or("no such node in the definition")?;
    let instance = graph.nodes[at].clone();
    let group = graph
        .groups
        .iter()
        .find(|group| group.name == instance.type_ref)
        .ok_or_else(|| {
            format!(
                "node {} instantiates `{}`, which no group in the document defines; there is nothing to unpack",
                instance.name(registry.node_type(&instance.type_ref)),
                instance.type_ref
            )
        })?
        .clone();

    // The uuids the returning nodes answer to: their body uuid unless a
    // node that survives the unpack already holds it, in which case a
    // fresh one — an invisible identity never refuses a visible gesture.
    let mut surviving: HashSet<Uuid> = graph.nodes.iter().map(|node| node.uuid).collect();
    surviving.remove(&uuid);
    let seated = stored_position(&instance);
    let mut mapping = HashMap::new();
    let mut returning: Vec<NodeInstance> = Vec::with_capacity(group.nodes.len());
    let mut returned: Vec<Uuid> = Vec::with_capacity(group.nodes.len());
    for body in &group.nodes {
        let fresh = if surviving.contains(&body.uuid) {
            Uuid::new_v4()
        } else {
            body.uuid
        };
        mapping.insert(body.uuid, fresh);
        let mut node = body.clone();
        node.uuid = fresh;
        // The body's stored position is an offset from where the instance
        // sat; a body node without one keeps none and lands by the view's
        // fallback. An instance never placed leaves the stored coordinate
        // as it is — the only spatial fact the document carries.
        if let (Some((sx, sy)), Some((ox, oy))) = (seated, stored_position(body)) {
            set_position(&mut node.metadata, sx + ox, sy + oy);
        }
        returning.push(node);
        returned.push(fresh);
    }

    // What the boundary carries back: the external wires re-attach through
    // the bindings, and a value the instance held on an exposed input
    // lands on the bound inner input. Either way the bound input ends up
    // fed from outside, so an inner edge already feeding it gives way —
    // the landing wire replaces what the input held, as a wire drop does.
    // A wire at a port name no binding declares has nothing to land on
    // and goes with the instance: the loader checks a document's shape,
    // not its port names, and compile names such a wire.
    let mut spliced = HashSet::new();
    let mut reattached: Vec<Edge> = Vec::new();
    for edge in &graph.edges {
        if edge.to == uuid {
            let Some(port) = group.inputs.iter().find(|port| port.name == edge.to_port) else {
                continue;
            };
            let to = mapping[&port.node];
            spliced.insert((to, port.port.clone()));
            reattached.push(Edge {
                from: edge.from,
                from_port: edge.from_port.clone(),
                to,
                to_port: port.port.clone(),
            });
        } else if edge.from == uuid {
            let Some(port) = group
                .outputs
                .iter()
                .find(|port| port.name == edge.from_port)
            else {
                continue;
            };
            reattached.push(Edge {
                from: mapping[&port.node],
                from_port: port.port.clone(),
                to: edge.to,
                to_port: edge.to_port.clone(),
            });
        }
    }
    for port in &group.inputs {
        if let Some(value) = instance.parameters.get(&port.name) {
            let node = mapping[&port.node];
            spliced.insert((node, port.port.clone()));
            returning
                .iter_mut()
                .find(|node| node.uuid == mapping[&port.node])
                .expect("an exposed port binds a node the group contains")
                .parameters
                .insert(port.port.clone(), value.clone());
        }
    }
    let inner: Vec<Edge> = group
        .edges
        .iter()
        .filter(|edge| !spliced.contains(&(mapping[&edge.to], edge.to_port.clone())))
        .map(|edge| Edge {
            from: mapping[&edge.from],
            from_port: edge.from_port.clone(),
            to: mapping[&edge.to],
            to_port: edge.to_port.clone(),
        })
        .collect();

    graph.nodes.remove(at);
    // The definition leaves with the instance that referenced it, unless
    // another instance — on the canvas now or inside another group —
    // still shows it.
    let name = group.name.clone();
    let referenced = graph.nodes.iter().any(|node| node.type_ref == name)
        || graph.groups.iter().any(|group| {
            group.name != name && group.nodes.iter().any(|node| node.type_ref == name)
        });
    if !referenced {
        graph.groups.retain(|group| group.name != name);
    }
    graph
        .edges
        .retain(|edge| edge.from != uuid && edge.to != uuid);
    graph.nodes.extend(returning);
    graph.edges.extend(reattached);
    graph.edges.extend(inner);
    Ok(returned)
}

/// The first node in the selection that would put a port on the boundary
/// without anything honest to declare on it — an unknown-typed placeholder
/// holding the in-selection end of a crossing edge, named the way an error
/// names it. A placeholder whose edges stay internal packages fine —
/// nobody asks its ports anything — and one outside the selection never
/// blocks, the packaged side's declarations being what the exposed port
/// carries.
fn boundary_placeholder(
    graph: &GraphDefinition,
    selected: &HashSet<Uuid>,
    registry: &Registry,
) -> Option<String> {
    for edge in &graph.edges {
        let from_in = selected.contains(&edge.from);
        let to_in = selected.contains(&edge.to);
        if from_in == to_in {
            continue;
        }
        let uuid = if from_in { edge.from } else { edge.to };
        let node = graph
            .nodes
            .iter()
            .find(|node| node.uuid == uuid)
            .expect("the selection was checked against the definition");
        let declared = registry.node_type(&node.type_ref).is_some()
            || graph.groups.iter().any(|group| group.name == node.type_ref);
        if !declared {
            return Some(node.name(registry.node_type(&node.type_ref)).to_owned());
        }
    }
    None
}

/// The exposed ports the selection's crossing edges derive, walking the
/// edges in their stored order, with each crossing edge rewritten to run
/// through the new instance instead — an input's single upstream re-seated,
/// a fan-out's every downstream kept. One inner output serves all its
/// external downstreams, so a binding already spoken for takes the first
/// port's name again; an inner input and an inner output sharing a name
/// are distinct ports, but the names they take are deduped across the
/// whole new port set.
fn exposed_ports(
    graph: &GraphDefinition,
    registry: &Registry,
    selected: &HashSet<Uuid>,
    instance: Uuid,
) -> (Vec<GroupPort>, Vec<GroupPort>, Vec<Edge>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut reattached = Vec::new();
    let mut taken = HashSet::new();
    let mut spoken: HashMap<(bool, Uuid, String), String> = HashMap::new();
    for edge in &graph.edges {
        let from_in = selected.contains(&edge.from);
        let to_in = selected.contains(&edge.to);
        let (outward, node, port) = if from_in && !to_in {
            (true, edge.from, &edge.from_port)
        } else if to_in && !from_in {
            (false, edge.to, &edge.to_port)
        } else {
            continue;
        };
        let name = match spoken.entry((outward, node, port.clone())) {
            Entry::Occupied(seen) => seen.get().clone(),
            Entry::Vacant(seat) => {
                let mut name = port.clone();
                let mut suffix = 2;
                while !taken.insert(name.clone()) {
                    name = format!("{port} {suffix}");
                    suffix += 1;
                }
                let ports = if outward { &mut outputs } else { &mut inputs };
                ports.push(GroupPort {
                    name: name.clone(),
                    type_refs: inner_port_types(graph, registry, node, port, !outward),
                    node,
                    port: port.clone(),
                });
                seat.insert(name.clone());
                name
            }
        };
        reattached.push(if outward {
            Edge {
                from: instance,
                from_port: name,
                to: edge.to,
                to_port: edge.to_port.clone(),
            }
        } else {
            Edge {
                from: edge.from,
                from_port: edge.from_port.clone(),
                to: instance,
                to_port: name,
            }
        });
    }
    (inputs, outputs, reattached)
}

/// The type references the bound inner port declares: a group instance's
/// exposed ports resolving against the document's groups before the
/// linked types' — the same resolution the canvas draws by. A port the
/// declaring type does not name declares nothing, compile's to judge.
fn inner_port_types(
    graph: &GraphDefinition,
    registry: &Registry,
    uuid: Uuid,
    port: &str,
    input: bool,
) -> Vec<String> {
    let instance = graph.nodes.iter().find(|node| node.uuid == uuid);
    let Some(instance) = instance else {
        return Vec::new();
    };
    let ports: Option<Vec<(String, Vec<String>)>> = if let Some(group) = graph
        .groups
        .iter()
        .find(|group| group.name == instance.type_ref)
    {
        let declared = if input { &group.inputs } else { &group.outputs };
        Some(
            declared
                .iter()
                .map(|port| (port.name.clone(), port.type_refs.clone()))
                .collect(),
        )
    } else if let Some(node_type) = registry.node_type(&instance.type_ref) {
        let declared = if input {
            node_type.inputs
        } else {
            node_type.outputs
        };
        Some(
            declared
                .iter()
                .map(|port| {
                    (
                        port.name.to_owned(),
                        port.type_refs.iter().map(|r| r.to_string()).collect(),
                    )
                })
                .collect(),
        )
    } else {
        None
    };
    ports
        .and_then(|ports| ports.into_iter().find(|(name, _)| name == port))
        .map(|(_, refs)| refs)
        .unwrap_or_default()
}

/// The average of the selection's stored positions; `None` when no node
/// in the selection carries one, so the instance carries none either.
fn centroid(graph: &GraphDefinition, selected: &HashSet<Uuid>) -> Option<(f64, f64)> {
    let mut count = 0usize;
    let mut sum = (0.0, 0.0);
    for node in &graph.nodes {
        if !selected.contains(&node.uuid) {
            continue;
        }
        if let Some((x, y)) = stored_position(node) {
            sum.0 += x;
            sum.1 += y;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some((sum.0 / count as f64, sum.1 / count as f64))
}

/// The node's stored position, read the way the view reads it: a
/// `position` metadata entry carrying numeric `x` and `y`.
fn stored_position(node: &NodeInstance) -> Option<(f64, f64)> {
    let position = node.metadata.get("position")?.as_mapping()?;
    let coordinate = |axis: &str| {
        position
            .iter()
            .find(|(key, _)| key.as_str() == Some(axis))
            .and_then(|(_, value)| value.as_f64())
    };
    Some((coordinate("x")?, coordinate("y")?))
}

/// Record a position in a node's metadata, replacing the stored one and
/// leaving the rest of the node's bookkeeping alone. A whole coordinate
/// stores as an integer, so a file a gesture writes reads as one a hand
/// writes.
fn set_position(metadata: &mut Mapping, x: f64, y: f64) {
    let mut position = Mapping::new();
    position.insert("x".into(), coordinate(x));
    position.insert("y".into(), coordinate(y));
    metadata.insert("position".into(), position.into());
}

fn position_metadata(x: f64, y: f64) -> Mapping {
    let mut metadata = Mapping::new();
    set_position(&mut metadata, x, y);
    metadata
}

fn coordinate(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() < 9.0e15 {
        Value::Number(serde_yaml::Number::from(value as i64))
    } else {
        Value::Number(serde_yaml::Number::from(value))
    }
}
