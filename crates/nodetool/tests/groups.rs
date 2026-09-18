//! Groups and subgraphs: the file format's `groups` section, the load
//! errors that are document-local, the compile-time flattening — derived
//! identities, boundary rewiring, a literal through the boundary, type
//! resolution across it — and the events a grouped run tells, named so an
//! inside mistake is findable from the outside.

use std::collections::BTreeMap;

use nodetool::compile::{self, inner_identity};
use nodetool::engine::{Event, Run};
use nodetool::graph::{
    self, Edge, GraphDefinition, GroupDefinition, GroupPort, LoadLocation, NodeInstance,
    ParameterValue, SCHEMA_VERSION,
};
use nodetool::registry::Registry;
use test_plugin_delta as _;
use test_plugin_gamma as _;
use uuid::Uuid;

const SOURCE: &str = "00000000-0000-0000-0000-0000000000a1";
const SINK: &str = "00000000-0000-0000-0000-0000000000b1";
const SINK_F64: &str = "00000000-0000-0000-0000-0000000000b2";
const COUNTER: &str = "00000000-0000-0000-0000-0000000000d1";
const INSTANCE: &str = "00000000-0000-0000-0000-0000000000e1";
const OUTER: &str = "00000000-0000-0000-0000-0000000000e2";
const INNER: &str = "00000000-0000-0000-0000-0000000000f1";
const FAILER: &str = "00000000-0000-0000-0000-0000000000f2";
const NESTED: &str = "00000000-0000-0000-0000-0000000000f3";
const DEEPEST: &str = "00000000-0000-0000-0000-0000000000f4";

fn parse(uuid: &str) -> Uuid {
    uuid.parse().expect("the test carries a valid uuid")
}

fn node(uuid: &str, type_ref: &str) -> NodeInstance {
    NodeInstance {
        uuid: parse(uuid),
        type_ref: type_ref.to_owned(),
        label: None,
        parameters: BTreeMap::new(),
        metadata: Default::default(),
    }
}

fn with_parameter(mut node: NodeInstance, name: &str, value: ParameterValue) -> NodeInstance {
    node.parameters.insert(name.to_owned(), value);
    node
}

fn edge(from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
    Edge {
        from: parse(from),
        from_port: from_port.to_owned(),
        to: parse(to),
        to_port: to_port.to_owned(),
    }
}

fn port(name: &str, type_refs: &[&str], node: &str, port: &str) -> GroupPort {
    GroupPort {
        name: name.to_owned(),
        type_refs: type_refs.iter().map(|name| name.to_string()).collect(),
        node: parse(node),
        port: port.to_owned(),
    }
}

fn group(
    name: &str,
    inputs: Vec<GroupPort>,
    outputs: Vec<GroupPort>,
    nodes: Vec<NodeInstance>,
    edges: Vec<Edge>,
) -> GroupDefinition {
    GroupDefinition {
        name: name.to_owned(),
        inputs,
        outputs,
        nodes,
        edges,
    }
}

fn grouped(
    nodes: Vec<NodeInstance>,
    edges: Vec<Edge>,
    groups: Vec<GroupDefinition>,
) -> GraphDefinition {
    GraphDefinition {
        schema_version: SCHEMA_VERSION,
        name: None,
        nodes,
        edges,
        groups,
    }
}

fn compile_ok(definition: &GraphDefinition) -> compile::CompiledGraph {
    compile::compile(definition, &Registry::collect())
        .graph
        .expect("the definition compiles")
}

fn compile_errors(definition: &GraphDefinition) -> Vec<(String, Vec<Uuid>)> {
    let result = compile::compile(definition, &Registry::collect());
    assert!(result.graph.is_none(), "the definition compiles");
    result
        .errors
        .iter()
        .map(|problem| (problem.message.clone(), problem.nodes.clone()))
        .collect()
}

fn single_error(mut errors: Vec<(String, Vec<Uuid>)>) -> (String, Vec<Uuid>) {
    assert_eq!(errors.len(), 1, "expected one error, got: {errors:?}");
    errors.remove(0)
}

/// The identity the inner node of the `double stage` instance derives.
fn stage_identity(inner: &str) -> Uuid {
    inner_identity(&[parse(INSTANCE)], parse(inner))
}

/// A group wrapping one `gamma/doubler`: the exposed input and output both
/// bind the doubler's `value`.
fn double_stage() -> GroupDefinition {
    group(
        "double stage",
        vec![port("value", &["i32"], INNER, "value")],
        vec![port("value", &["i32"], INNER, "value")],
        vec![node(INNER, "gamma/doubler")],
        vec![],
    )
}

#[test]
fn a_group_instance_compiles_to_its_inner_graph() {
    let compiled = compile_ok(&grouped(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(INSTANCE, "double stage"),
            node(SINK, "gamma/int_sink"),
        ],
        vec![
            edge(SOURCE, "value", INSTANCE, "value"),
            edge(INSTANCE, "value", SINK, "value"),
        ],
        vec![double_stage()],
    ));

    let identity = stage_identity(INNER);
    assert_eq!(compiled.nodes.len(), 3);
    assert!(compiled.nodes.contains_key(&parse(SOURCE)));
    assert!(compiled.nodes.contains_key(&parse(SINK)));
    let inner = &compiled.nodes[&identity];
    // The collapsed instance is gone; its inner node sits under the derived
    // identity, labelled by the instance it belongs to.
    assert_eq!(inner.node_type.type_ref, "gamma/doubler");
    assert_eq!(inner.label, "double stage · Doubler");

    // The boundary edges rewired: the external upstream feeds the inner
    // port, the inner output feeds the external downstream.
    assert_eq!(compiled.connections.len(), 2);
    assert_eq!(compiled.connections[0].from.to_string(), SOURCE);
    assert_eq!(compiled.connections[0].from_port, "value");
    assert_eq!(compiled.connections[0].to, identity);
    assert_eq!(compiled.connections[0].to_port, "value");
    assert_eq!(compiled.connections[0].resolved_type.name, "i32");
    assert!(compiled.connections[0].conversion.is_none());
    assert_eq!(compiled.connections[1].from, identity);
    assert_eq!(compiled.connections[1].from_port, "value");
    assert_eq!(compiled.connections[1].to.to_string(), SINK);
    assert_eq!(compiled.connections[1].to_port, "value");
}

#[test]
fn a_grouped_graph_compiles_like_the_same_graph_written_flat() {
    let identity = stage_identity(INNER);
    let packed = compile_ok(&grouped(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(INSTANCE, "double stage"),
            node(SINK, "gamma/int_sink"),
        ],
        vec![
            edge(SOURCE, "value", INSTANCE, "value"),
            edge(INSTANCE, "value", SINK, "value"),
        ],
        vec![double_stage()],
    ));
    let mut inner = node(INNER, "gamma/doubler");
    inner.uuid = identity;
    let flat = compile_ok(&grouped(
        vec![
            node(SOURCE, "gamma/int_source"),
            inner,
            node(SINK, "gamma/int_sink"),
        ],
        vec![
            edge(SOURCE, "value", &identity.to_string(), "value"),
            edge(&identity.to_string(), "value", SINK, "value"),
        ],
        vec![],
    ));

    assert_eq!(
        format!("{:?}", packed.connections),
        format!("{:?}", flat.connections)
    );
    for (uuid, node) in &packed.nodes {
        let twin = &flat.nodes[uuid];
        assert_eq!(node.node_type.type_ref, twin.node_type.type_ref);
        assert_eq!(
            format!("{:?}", node.parameters),
            format!("{:?}", twin.parameters)
        );
        assert_eq!(format!("{:?}", node.fed), format!("{:?}", twin.fed));
    }
}

#[test]
fn an_unconnected_exposed_input_carries_a_parameter_through_the_boundary() {
    let compiled = compile_ok(&grouped(
        vec![with_parameter(
            node(INSTANCE, "double stage"),
            "value",
            ParameterValue::Int(3),
        )],
        vec![],
        vec![double_stage()],
    ));

    let identity = stage_identity(INNER);
    let parameter = &compiled.nodes[&identity].parameters["value"];
    assert_eq!(parameter.resolved_type.name, "i32");
    assert_eq!(parameter.value.get::<i32>(), Some(&3));
    assert!(compiled.connections.is_empty());
}

#[test]
fn types_resolve_across_the_boundary_under_the_ordinary_rules() {
    // The exposed input declares i32; the port it binds declares f64 — the
    // declared i32→f64 conversion bridges the interface, and the external
    // connection rides it like any edge's.
    let stage = group(
        "as float",
        vec![port("value", &["i32"], SINK_F64, "value")],
        vec![],
        vec![node(SINK_F64, "gamma/float64_sink")],
        vec![],
    );
    let compiled = compile_ok(&grouped(
        vec![node(SOURCE, "gamma/int_source"), node(INSTANCE, "as float")],
        vec![edge(SOURCE, "value", INSTANCE, "value")],
        vec![stage],
    ));

    assert_eq!(compiled.connections.len(), 1);
    let connection = &compiled.connections[0];
    assert_eq!(
        connection.to,
        inner_identity(&[parse(INSTANCE)], parse(SINK_F64))
    );
    assert_eq!(connection.resolved_type.name, "f64");
    assert!(
        connection.conversion.is_some(),
        "the conversion rides the boundary"
    );
}

#[test]
fn a_group_may_instantiate_another_group() {
    // outer (instance OUTER) → inner (instance NESTED) → doubler DEEPEST:
    // the deepest node's identity derives from the whole chain.
    let inner = group(
        "inner stage",
        vec![port("value", &["i32"], DEEPEST, "value")],
        vec![port("value", &["i32"], DEEPEST, "value")],
        vec![node(DEEPEST, "gamma/doubler")],
        vec![],
    );
    let outer = group(
        "outer stage",
        vec![port("value", &["i32"], NESTED, "value")],
        vec![port("value", &["i32"], NESTED, "value")],
        vec![node(NESTED, "inner stage")],
        vec![],
    );
    let compiled = compile_ok(&grouped(
        vec![
            node(SOURCE, "gamma/int_source"),
            node(INSTANCE, "outer stage"),
            node(SINK, "gamma/int_sink"),
        ],
        vec![
            edge(SOURCE, "value", INSTANCE, "value"),
            edge(INSTANCE, "value", SINK, "value"),
        ],
        vec![inner, outer],
    ));

    let identity = inner_identity(&[parse(INSTANCE), parse(NESTED)], parse(DEEPEST));
    assert!(compiled.nodes.contains_key(&identity));
    assert_eq!(compiled.connections.len(), 2);
    assert_eq!(compiled.connections[0].to, identity);
    assert_eq!(compiled.connections[1].from, identity);
    assert_eq!(compiled.connections[1].to.to_string(), SINK);
}

#[test]
fn an_instance_uuid_reused_under_different_chains_derives_distinct_identities() {
    // The same inner node uuid, and the same inner instance uuid, legally
    // reused across the two groups' graphs: the chains tell them apart.
    let alone = group(
        "alone",
        vec![],
        vec![],
        vec![node(INNER, "gamma/doubler")],
        vec![],
    );
    let nested = group(
        "nested",
        vec![],
        vec![],
        vec![node(INSTANCE, "alone")],
        vec![],
    );
    let compiled = compile_ok(&grouped(
        vec![node(INSTANCE, "alone"), node(OUTER, "nested")],
        vec![],
        vec![alone, nested],
    ));

    let direct = inner_identity(&[parse(INSTANCE)], parse(INNER));
    let through = inner_identity(&[parse(OUTER), parse(INSTANCE)], parse(INNER));
    assert_ne!(direct, through);
    assert!(compiled.nodes.contains_key(&direct));
    assert!(compiled.nodes.contains_key(&through));
    assert_eq!(compiled.nodes.len(), 2);
}

#[test]
fn a_group_reference_cycle_is_an_error_wherever_it_sits() {
    // Neither group is instantiated: the cycle alone ends the compile.
    let a = group("a", vec![], vec![], vec![node(INNER, "b")], vec![]);
    let b = group("b", vec![], vec![], vec![node(INNER, "a")], vec![]);
    let (message, nodes) = single_error(compile_errors(&grouped(vec![], vec![], vec![a, b])));
    assert!(message.contains("group cycle"), "{message}");
    assert!(message.contains("a → b → a"), "{message}");
    assert!(nodes.is_empty());
}

#[test]
fn a_group_name_colliding_with_a_plugin_type_is_an_error() {
    let stage = group(
        "gamma/int_sink",
        vec![],
        vec![],
        vec![node(INNER, "gamma/doubler")],
        vec![],
    );
    let (message, nodes) = single_error(compile_errors(&grouped(
        vec![node(INSTANCE, "gamma/int_sink")],
        vec![],
        vec![stage],
    )));
    assert!(
        message.contains("group `gamma/int_sink` collides with a node type"),
        "{message}"
    );
    assert_eq!(nodes, vec![parse(INSTANCE)]);
}

#[test]
fn a_binding_naming_an_inner_port_the_node_does_not_have_is_an_error() {
    let stage = group(
        "double stage",
        vec![port("value", &["i32"], INNER, "nope")],
        vec![],
        vec![node(INNER, "gamma/doubler")],
        vec![],
    );
    let (message, nodes) = single_error(compile_errors(&grouped(
        vec![node(INSTANCE, "double stage")],
        vec![],
        vec![stage],
    )));
    assert!(
        message.contains("group `double stage`")
            && message.contains("port `nope`")
            && message.contains("does not declare it"),
        "{message}"
    );
    assert_eq!(nodes, vec![parse(INSTANCE)]);
}

#[test]
fn an_exposed_port_whose_types_cannot_bridge_to_its_binding_is_an_error() {
    let stage = group(
        "double stage",
        vec![port("value", &["String"], INNER, "value")],
        vec![],
        vec![node(INNER, "gamma/doubler")],
        vec![],
    );
    let (message, nodes) = single_error(compile_errors(&grouped(
        vec![node(INSTANCE, "double stage")],
        vec![],
        vec![stage],
    )));
    assert!(
        message.contains("exposed input port `value` declares String")
            && message.contains("cannot bridge"),
        "{message}"
    );
    assert_eq!(nodes, vec![parse(INSTANCE)]);
}

#[test]
fn an_instance_referencing_an_undefined_group_is_an_error() {
    let (message, nodes) = single_error(compile_errors(&grouped(
        vec![node(INSTANCE, "no/such group")],
        vec![],
        vec![],
    )));
    assert!(
        message.contains("no group in the document defines")
            && message.contains("no linked plugin declares"),
        "{message}"
    );
    assert_eq!(nodes, vec![parse(INSTANCE)]);
}

#[test]
fn a_problem_inside_a_group_marks_the_instance_the_canvas_shows() {
    // The inner sink's literal does not fit its port: the error names the
    // inner node by its composed label, and the mark lands on the group
    // instance.
    let stage = group(
        "double stage",
        vec![],
        vec![],
        vec![with_parameter(
            node(INNER, "gamma/int_sink"),
            "value",
            ParameterValue::Str("not a number".to_owned()),
        )],
        vec![],
    );
    let (message, nodes) = single_error(compile_errors(&grouped(
        vec![node(INSTANCE, "double stage")],
        vec![],
        vec![stage],
    )));
    assert!(message.contains("double stage · Int sink"), "{message}");
    assert_eq!(nodes, vec![parse(INSTANCE)]);
}

#[test]
fn a_group_cycle_through_a_boundary_edge_ends_the_compile() {
    // The flattening turns a boundary-crossing edge into an ordinary one,
    // so a cycle across the boundary is the graph cycle it always was.
    let stage = group(
        "double stage",
        vec![port("value", &["i32"], INNER, "value")],
        vec![port("value", &["i32"], INNER, "value")],
        vec![node(INNER, "gamma/passthrough")],
        vec![],
    );
    let (message, nodes) = single_error(compile_errors(&grouped(
        vec![node(INSTANCE, "double stage")],
        vec![edge(INSTANCE, "value", INSTANCE, "value")],
        vec![stage],
    )));
    assert!(message.contains("cycle"), "{message}");
    assert_eq!(nodes, vec![parse(INSTANCE)]);
}

// The file format: round trip and the document-local load errors.

const GROUPED_FILE: &str = "schema_version: 2
name: grouped
nodes:
  - uuid: 00000000-0000-0000-0000-0000000000e1
    type_ref: double stage
    label: The stage
    parameters:
      value: 3
    metadata:
      position: { x: 80, y: 120 }
  - uuid: 00000000-0000-0000-0000-0000000000b1
    type_ref: gamma/int_sink
edges:
  - from: 00000000-0000-0000-0000-0000000000e1
    from_port: value
    to: 00000000-0000-0000-0000-0000000000b1
    to_port: value
groups:
  - name: double stage
    inputs:
      - name: value
        type_refs: [i32]
        node: 00000000-0000-0000-0000-0000000000f1
        port: value
    outputs:
      - name: value
        type_refs: [i32]
        node: 00000000-0000-0000-0000-0000000000f1
        port: value
    nodes:
      - uuid: 00000000-0000-0000-0000-0000000000f1
        type_ref: gamma/doubler
        label: The doubler
        metadata:
          position: { x: 40, y: 20 }
          layers: [a, b]
    edges: []
";

#[test]
fn a_group_round_trips_verbatim() {
    let definition = graph::load(GROUPED_FILE).unwrap();
    assert_eq!(definition.groups.len(), 1);
    let stage = &definition.groups[0];
    assert_eq!(stage.name, "double stage");
    assert_eq!(stage.inputs[0].name, "value");
    assert_eq!(stage.inputs[0].type_refs, vec!["i32".to_owned()]);
    assert_eq!(stage.inputs[0].node.to_string(), INNER);
    assert_eq!(stage.inputs[0].port, "value");
    assert_eq!(stage.outputs.len(), 1);
    assert_eq!(stage.nodes.len(), 1);
    // Inner metadata, labels included, carried verbatim.
    let inner = &stage.nodes[0];
    assert_eq!(inner.label.as_deref(), Some("The doubler"));
    let metadata = &inner.metadata;
    let position = metadata
        .get(graph::Value::String("position".into()))
        .unwrap();
    assert_eq!(
        position.get(graph::Value::String("x".into())),
        Some(&graph::Value::Number(40.into()))
    );
    let layers = metadata.get(graph::Value::String("layers".into())).unwrap();
    assert_eq!(layers.get(1), Some(&graph::Value::String("b".into())));
    // The instance is an ordinary node instance.
    assert_eq!(definition.nodes[0].type_ref, "double stage");
    assert_eq!(definition.nodes[0].label.as_deref(), Some("The stage"));

    let dumped = graph::dump(&definition);
    assert_eq!(graph::load(&dumped).unwrap(), definition);
    assert!(dumped.contains("groups:"), "dumped:\n{dumped}");
    assert!(dumped.contains("type_refs:"), "dumped:\n{dumped}");
}

#[test]
fn a_document_without_groups_dumps_without_the_section() {
    let definition = graph::load(
        "schema_version: 2\nnodes:\n  - uuid: 00000000-0000-0000-0000-0000000000a1\n    type_ref: t\nedges: []\n",
    )
    .unwrap();
    assert!(definition.groups.is_empty());
    let dumped = graph::dump(&definition);
    assert!(!dumped.contains("groups"), "dumped:\n{dumped}");
}

#[test]
fn a_version_1_document_cannot_carry_groups() {
    let (location, message) =
        match graph::load("schema_version: 1\nnodes: []\nedges: []\ngroups: []\n") {
            Err(error) => (error.location, error.message),
            Ok(_) => panic!("a version 1 document carrying groups loads"),
        };
    assert_eq!(location, LoadLocation::Path("schema_version".to_owned()));
    assert!(
        message.contains("version 1") && message.contains("groups"),
        "{message}"
    );
}

#[test]
fn a_group_name_is_unique_in_the_document() {
    let (location, message) = match graph::load(
        "schema_version: 2\nnodes: []\nedges: []\ngroups:\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes: []\n    edges: []\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes: []\n    edges: []\n",
    ) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("duplicate group names load"),
    };
    assert_eq!(location, LoadLocation::Path("groups[1]".to_owned()));
    assert!(
        message.contains("duplicate group name `stage`"),
        "{message}"
    );
}

#[test]
fn an_inner_edge_lands_only_on_its_own_group() {
    let (location, message) = match graph::load(&format!(
        "schema_version: 2\nnodes:\n  - uuid: {SINK}\n    type_ref: t\nedges: []\ngroups:\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes:\n      - uuid: {INNER}\n        type_ref: t\n    edges:\n      - from: {INNER}\n        from_port: a\n        to: {SINK}\n        to_port: b\n"
    )) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("an inner edge landing outside its group loads"),
    };
    assert_eq!(
        location,
        LoadLocation::Path("groups[0].edges[0]".to_owned())
    );
    assert!(
        message.contains("is not defined in group `stage`") && message.contains(SINK),
        "{message}"
    );
}

#[test]
fn a_binding_names_an_inner_node_the_group_contains() {
    let (location, message) = match graph::load(&format!(
        "schema_version: 2\nnodes: []\nedges: []\ngroups:\n  - name: stage\n    inputs:\n      - name: value\n        type_refs: [i32]\n        node: {SINK}\n        port: value\n    outputs: []\n    nodes:\n      - uuid: {INNER}\n        type_ref: t\n    edges: []\n"
    )) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("a binding outside its group loads"),
    };
    assert_eq!(
        location,
        LoadLocation::Path("groups[0].inputs[0]".to_owned())
    );
    assert!(
        message.contains("binds node")
            && message.contains(SINK)
            && message.contains("does not contain"),
        "{message}"
    );
}

#[test]
fn a_groups_inner_graph_carries_the_ordinary_cross_checks() {
    let yaml = format!(
        "schema_version: 2\nnodes: []\nedges: []\ngroups:\n  - name: stage\n    inputs: []\n    outputs: []\n    nodes:\n      - uuid: {INNER}\n        type_ref: t\n      - uuid: {INNER}\n        type_ref: t\n    edges: []\n"
    );
    let (location, message) = match graph::load(&yaml) {
        Err(error) => (error.location, error.message),
        Ok(_) => panic!("duplicate inner uuids load"),
    };
    assert_eq!(
        location,
        LoadLocation::Path("groups[0].nodes[1]".to_owned())
    );
    assert!(message.contains("duplicate node uuid"), "{message}");
}

#[test]
fn the_definition_carries_groups_to_the_browser_verbatim() {
    let definition = graph::load(GROUPED_FILE).unwrap();
    let carried = serde_json::to_value(&definition).unwrap();
    assert_eq!(carried["groups"][0]["name"], "double stage");
    assert_eq!(carried["groups"][0]["inputs"][0]["port"], "value");
    assert_eq!(
        carried["groups"][0]["nodes"][0]["metadata"]["position"]["x"],
        40
    );
    assert_eq!(carried["nodes"][0]["type_ref"], "double stage");
}

// The run: events named so the inside is findable from the outside.

/// Collects the run's timeline through a channel, the way the observer
/// tests read it.
fn timeline() -> (Timeline, tokio::sync::mpsc::UnboundedReceiver<Event>) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (Timeline { sender }, receiver)
}

struct Timeline {
    sender: tokio::sync::mpsc::UnboundedSender<Event>,
}

#[nodetool::async_trait]
impl nodetool::engine::Observer for Timeline {
    async fn observe(&self, event: Event) {
        let _ = self.sender.send(event);
    }
}

/// Reads the timeline up to and including run finished — the run's last
/// event, delivered in order.
async fn read_to_end(mut receiver: tokio::sync::mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Some(event) = receiver.recv().await {
        let finished = matches!(event, Event::RunFinished { .. });
        events.push(event);
        if finished {
            return events;
        }
    }
    events
}

fn started(events: &[Event], uuid: Uuid) -> Option<&Event> {
    events
        .iter()
        .find(|event| matches!(event, Event::NodeStarted { node } if node.uuid == uuid))
}

#[tokio::test]
async fn inner_events_carry_derived_identities_and_composed_labels() {
    let compiled = compile_ok(&grouped(
        vec![
            node(COUNTER, "delta/counter"),
            node(INSTANCE, "double stage"),
        ],
        vec![edge(COUNTER, "out", INSTANCE, "value")],
        vec![double_stage()],
    ));
    let (observer, receiver) = timeline();
    let mut run = Run::new(&compiled);
    run.observe(std::sync::Arc::new(observer));
    run.start().await.expect("the run completes");

    let events = read_to_end(receiver).await;
    let identity = stage_identity(INNER);
    let started = started(&events, identity).expect("the inner node started");
    match started {
        Event::NodeStarted { node } => {
            assert_eq!(node.label, "double stage · Doubler");
        }
        _ => unreachable!(),
    }
    // The counter feeding the boundary and the doubler emitting across it:
    // every doubled value arrives, named by the derived identity.
    let doubled: Vec<i32> = events
        .iter()
        .filter_map(|event| match event {
            Event::Emitted { node, port, value } if node.uuid == identity && *port == "value" => {
                value.get::<i32>().copied()
            }
            _ => None,
        })
        .collect();
    assert_eq!(doubled, (1..=50).map(|value| value * 2).collect::<Vec<_>>());
}

#[tokio::test]
async fn a_failing_inner_node_ends_the_run_naming_the_group_instance_and_the_inner_node() {
    let failer_stage = group(
        "double stage",
        vec![port("value", &["i32"], FAILER, "value")],
        vec![],
        vec![node(FAILER, "delta/failer")],
        vec![],
    );
    let compiled = compile_ok(&grouped(
        vec![
            node(COUNTER, "delta/counter"),
            node(INSTANCE, "double stage"),
        ],
        vec![edge(COUNTER, "out", INSTANCE, "value")],
        vec![failer_stage],
    ));
    let (observer, receiver) = timeline();
    let mut run = Run::new(&compiled);
    run.observe(std::sync::Arc::new(observer));
    let error = run.start().await.expect_err("the run fails");

    let identity = stage_identity(FAILER);
    let report = error.to_string();
    assert!(
        report.contains("double stage · Failer") && report.contains(&identity.to_string()),
        "{report}"
    );
    // The outcome names the node the failure belongs to — the derived
    // identity, recoverable to the group instance by the same fold.
    let events = read_to_end(receiver).await;
    let finished = events.last().expect("run finished arrives");
    match finished {
        Event::RunFinished { outcome } => match outcome {
            nodetool::engine::RunOutcome::Failed {
                node: Some(node), ..
            } => {
                assert_eq!(node.uuid, identity);
            }
            other => panic!("expected a failed outcome, got {other:?}"),
        },
        _ => unreachable!(),
    }
}
