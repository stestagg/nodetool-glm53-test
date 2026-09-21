//! The server side of the editor: a small HTTP server that serves the
//! embedded UI over one address and speaks the JSON envelope protocol over
//! one websocket connection per browser.
//!
//! The server owns the graph, the file it belongs to, and the run of the
//! graph. It holds the in-memory definition — the same document the files
//! carry and the compiler takes — and the browser is a synced view of it:
//! every edit is an operation applied here, and the whole updated
//! definition is pushed to every connection, so a reload or a second tab
//! simply asks for the definition again. The file state — the path being
//! edited and whether the definition has unsaved changes — travels beside
//! the definition wherever the definition travels, and is pushed alone
//! when a save changes it without touching the definition. The run state
//! — whether a run is on and how the last one ended — travels beside them
//! the same way, and with it the run display: the per-node status and the
//! latest value per output port the bridge derives from the
//! run's own events and holds beside the rest. The display rides the push
//! channel — a connecting tab is given it as a push of its own, on the
//! one ordered stream every later status change and emission rides — so a
//! browser connecting at any time is given the canvas as it stands. The
//! run's events themselves
//! cross to every connection as they occur, forwarded by the bridge, the
//! one observer the editor subscribes to the engine with. So do the
//! problems: after every change to the held
//! definition, and when a file is opened, the same compile a start runs
//! recomputes the graph's problems — errors and non-fatal warnings, each
//! naming the nodes it speaks of — and they are held as state like the
//! definition, travelling with it wherever it travels, the editor's early
//! warning that advises but never polices, enforcement staying compile's
//! at start. One graph state, not two; edits land last-write-wins,
//! with no locking, presence, or merge ceremony, for a local single-user
//! tool. While a run is on, the definition is held still: the UI quiets
//! its editing controls and the server refuses any edit operation that
//! arrives, the one rule covering every editing operation. Starting a run
//! compiles the held definition afresh — every start compiles, nothing
//! compiled survives a run — and the run's endings come back through the
//! engine's event stream. What a run's values reach beyond the canvas is
//! the host's: the taps a hosting binary names by its own plugin's node
//! types, defined at `TapConsumer`. Opening and saving go through the one
//! file format's loader and dump, so a file the headless run takes is the
//! file the editor edits.
//!
//! The websocket is untrusted input at a parse boundary: a message that
//! fails to parse, is not a JSON object, names an unknown type, or carries
//! unknown fields is answered with an error naming the problem, the
//! connection stays open, and nothing can crash or corrupt the server.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, watch};

use crate::compile;
use crate::graph::{self, Edge, GraphDefinition};
use crate::registry::Registry;
use crate::NodeType;
use uuid::Uuid;

mod assets;
mod bridge;
mod http;
mod packaging;
mod protocol;
mod run;
mod ws;

pub use protocol::{greeting, PROTOCOL_VERSION};
pub use run::{Outcome, RunState, TapConsumer, TapStream};

/// The address the editor serves on by default: loopback only — this is a
/// local tool.
pub const DEFAULT_ADDRESS: &str = "127.0.0.1:8420";

/// One message queued for a connection's write half.
enum Outbound {
    Text(String),
    Pong(Vec<u8>),
}

/// The editing session: the held definition, the file it belongs to —
/// none while the graph is untitled — whether the definition has
/// changed since that file was last opened or saved, the run of the
/// graph, the problems the compiler finds in the held definition, and
/// the run display the bridge derives from the run's events. One lock
/// over the six, so a file operation cannot interleave with an edit, a
/// run cannot start between an edit's check and its application, and
/// every push leaves in the order it became true.
struct Session {
    graph: GraphDefinition,
    file: Option<String>,
    dirty: bool,
    run: RunState,
    /// Which run the session is on: bumped as each run starts, and again
    /// when a reset ends one out of band. A run's observer carries the
    /// generation it was spawned with and is heard only while it is still
    /// the session's, so a run the reset already ended cannot speak its
    /// ending back into a session that has moved past it.
    generation: u64,
    problems: Vec<compile::Problem>,
    display: bridge::RunDisplay,
}

/// The editing server: the held session, the palette listing of every
/// linked plugin's node types, the registry the start compiles against and
/// the bridge serialises values through, the plugin-declared UI assets,
/// the push channel every connection rides, and the optional consumer the
/// hosting binary supplies for the runs' unconnected outputs.
pub struct Editor {
    session: Arc<Mutex<Session>>,
    listing: Vec<&'static NodeType>,
    registry: Arc<Registry>,
    base_scalars: serde_json::Map<String, Value>,
    data_types: serde_json::Map<String, Value>,
    plugin_assets: Vec<assets::PluginAsset>,
    pushes: broadcast::Sender<String>,
    taps: Vec<run::Tap>,
}

/// The message types that edit the held definition — the node operations
/// and the file operations with them. Refused while a run is on; looking
/// (the listing, the definition) stays open. The dispatch in `handle`
/// names the same kinds: an editing operation wired there without being
/// listed here slips past the refusal.
const EDITING: &[&str] = &[
    "create_node",
    "move_node",
    "set_label",
    "set_parameter",
    "wire",
    "unhook",
    "delete_node",
    "package_group",
    "unpack_group",
    "open_file",
    "save_file",
    "new_graph",
];

impl Editor {
    /// An editor holding `definition` and the file it came from, if any —
    /// `None` for a fresh, untitled graph. Launching on a graph file hands
    /// the loader's definition and the file's path here; the definition is
    /// clean until an edit lands, no run is on, and the definition's
    /// problems are the compile's, computed before the first connection
    /// asks.
    pub fn new(definition: GraphDefinition, file: Option<String>) -> Editor {
        let (pushes, _) = broadcast::channel(64);
        let registry = Arc::new(Registry::collect());
        let problems = problems_of(&definition, &registry);
        let listing = protocol::node_type_listing();
        let base_scalars = protocol::base_scalars();
        let data_types = protocol::data_type_facts(&listing);
        let plugin_assets = assets::plugin_assets();
        Editor {
            session: Arc::new(Mutex::new(Session {
                graph: definition,
                file,
                dirty: false,
                run: RunState::Idle { outcome: None },
                generation: 0,
                problems,
                display: bridge::RunDisplay::default(),
            })),
            listing,
            registry,
            base_scalars,
            data_types,
            plugin_assets,
            pushes,
            taps: Vec::new(),
        }
    }

    /// The session under its one lock: every read and edit holds the
    /// guard for the moment of its operation, the lock never poisoned.
    fn session(&self) -> MutexGuard<'_, Session> {
        self.session
            .lock()
            .expect("the session lock is never poisoned")
    }

    /// Listen to the runs started from here: every value an instance of
    /// `type_ref` receives on its `port` input reaches a stream `consumer`
    /// builds, one stream per instance, resolved afresh against each run's
    /// compiled graph. The host's seam — `TapConsumer`'s contract — and the
    /// host names its interest by its own plugin's types, core naming none.
    /// Taps accumulate; a host naming none leaves the runs unlistened to.
    ///
    /// A `port` the named node type does not declare as an input is a bug
    /// in the hosting binary, and the run it builds panics naming it.
    pub fn tap(
        mut self,
        type_ref: &'static str,
        port: &'static str,
        consumer: TapConsumer,
    ) -> Editor {
        self.taps.push(run::Tap {
            type_ref,
            port,
            consumer,
        });
        self
    }

    /// Whether the held definition has changes no file holds — the dirty
    /// state the file state carries, read here by the process hosting the
    /// editor for its exit guard.
    pub fn unsaved_changes(&self) -> bool {
        self.session().dirty
    }

    /// Serve connections on `listener` forever: UI assets over HTTP, the
    /// protocol over the websocket endpoint. A failed connection is
    /// reported and never ends the server.
    pub async fn serve(self: Arc<Self>, listener: tokio::net::TcpListener) -> io::Result<()> {
        loop {
            let (stream, _) = listener.accept().await?;
            let editor = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(error) = editor.accept(stream).await {
                    eprintln!("editor connection failed: {error}");
                }
            });
        }
    }

    /// Receive every push from here on: the seam an observer of the push
    /// channel subscribes through, as a connection does on connect.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.pushes.subscribe()
    }

    /// Answer one protocol message with its reply: the envelope's single
    /// entry point, so a connection is one call per received text. Always
    /// answers — a message that parses but cannot be applied, like a
    /// malformed one, gets an error reply naming the problem, and the
    /// connection carries on.
    pub fn handle(&self, message: &str) -> String {
        let mut fields = match serde_json::from_str::<Value>(message) {
            Ok(Value::Object(fields)) => fields,
            Ok(_) => return protocol::error_reply(None, "a message must be a JSON object"),
            Err(error) => return protocol::error_reply(None, &format!("not valid JSON: {error}")),
        };
        let id = fields.remove("id");
        let kind = match fields.remove("type") {
            Some(Value::String(kind)) => kind,
            Some(_) => return protocol::error_reply(id, "the message's `type` must be a string"),
            None => return protocol::error_reply(id, "a message must carry a `type`"),
        };
        if id.is_none() {
            return protocol::error_reply(None, "a message must carry an `id`");
        }
        // While a run is on the definition is held still, and the server
        // backs the lock the UI shows: any edit operation that arrives is
        // refused, naming the running state, before it can touch anything.
        // One rule, covering every editing operation — the node operations
        // and the file ones with them. The guard is held from this check
        // through the dispatch below, so an edit's check and its
        // application are one acquisition and a start cannot slip between
        // them.
        let mut session = self.session();
        if EDITING.contains(&kind.as_str()) && session.run.running() {
            return protocol::error_reply(
                id,
                "a run is on: the definition is held still until it ends",
            );
        }
        let reply = match kind.as_str() {
            "list_node_types" => self.list_node_types(&mut fields),
            "get_definition" => self.get_definition(&session, &mut fields),
            "create_node" => self.create_node(&mut session, &mut fields),
            "move_node" => self.move_node(&mut session, &mut fields),
            "set_label" => self.set_label(&mut session, &mut fields),
            "set_parameter" => self.set_parameter(&mut session, &mut fields),
            "wire" => self.wire(&mut session, &mut fields),
            "unhook" => self.unhook(&mut session, &mut fields),
            "delete_node" => self.delete_node(&mut session, &mut fields),
            "package_group" => self.package_group(&mut session, &mut fields),
            "unpack_group" => self.unpack_group(&mut session, &mut fields),
            "open_file" => self.open_file(&mut session, &mut fields),
            "save_file" => self.save_file(&mut session, &mut fields),
            "new_graph" => self.new_graph(&mut session, &mut fields),
            "start_run" => self.start_run(&mut session, &mut fields),
            "stop_run" => self.stop_run(&session, &mut fields),
            "reset_run" => self.reset_run(&mut session, &mut fields),
            other => Err(format!("unknown message type `{other}`")),
        };
        match reply {
            Ok(mut reply) => {
                reply["id"] = id.expect("checked above");
                reply.to_string()
            }
            Err(message) => protocol::error_reply(id, &message),
        }
    }

    fn list_node_types(
        &self,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        protocol::done(fields)?;
        let listing: Vec<Value> = self
            .listing
            .iter()
            .map(|t| protocol::node_type_json(t))
            .collect();
        Ok(json!({
            "type": "node_types",
            "node_types": listing,
            "base_scalars": self.base_scalars,
            "data_types": self.data_types,
        }))
    }

    fn get_definition(
        &self,
        session: &Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        protocol::done(fields)?;
        let graph = serde_json::to_value(&session.graph)
            .map_err(|error| format!("the held definition cannot be carried as JSON: {error}"))?;
        // The run display rides the push channel, not this reply: pushes
        // are sent under the session lock and read by each connection in
        // the order sent, so the snapshot a connecting tab joins with is
        // ordered against every later status change and emission. A reply
        // could not promise that — the reply and the pushes share one
        // write pump with no order between them, and a push sent after the
        // reply was composed can reach the browser first, leaving the
        // older snapshot it carries to revert state the tab has applied —
        // durable for statuses, which are pushed once per transition.
        self.push_display(session);
        Ok(json!({
            "type": "definition",
            "graph": graph,
            "file": protocol::file_state(session.file.as_deref(), session.dirty),
            "run": protocol::run_state(&session.run),
            "problems": protocol::problems_state(&session.problems),
        }))
    }

    fn create_node(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let type_ref = protocol::take_string(fields, "type_ref")?;
        let position = protocol::take_position(fields, "position")?;
        protocol::done(fields)?;
        // The reference resolves against the document's groups before the
        // node-type listing's — no ambiguity judged at edit time.
        let defined = session
            .graph
            .groups
            .iter()
            .any(|group| group.name == type_ref)
            || self
                .listing
                .iter()
                .any(|node_type| node_type.type_ref == type_ref);
        if !defined {
            return Err(format!(
                "no group `{type_ref}` is defined in this document and no node type `{type_ref}` \
                 is linked into this binary"
            ));
        }
        let uuid = uuid::Uuid::new_v4();
        let mut node = crate::graph::NodeInstance {
            uuid,
            type_ref,
            label: None,
            parameters: Default::default(),
            metadata: Default::default(),
        };
        node.metadata
            .insert("position".into(), position.metadata_entry().into());
        session.graph.nodes.push(node);
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "node_created", "uuid": uuid }))
    }

    fn move_node(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let position = protocol::take_position(fields, "position")?;
        protocol::done(fields)?;
        // The move records where the node rests; the rest of its metadata
        // is the node's own bookkeeping and stays untouched.
        Self::node_mut(&mut session.graph, uuid)?
            .metadata
            .insert("position".into(), position.metadata_entry().into());
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "node_moved" }))
    }

    /// The node an operation names, required to be in the definition. A
    /// reference naming none is a reference, not a node: the refusal says
    /// so without echoing the uuid it was handed.
    fn node(graph: &GraphDefinition, uuid: Uuid) -> Result<&crate::graph::NodeInstance, String> {
        graph
            .nodes
            .iter()
            .find(|node| node.uuid == uuid)
            .ok_or_else(|| "no such node in the definition".to_owned())
    }

    /// The same node, to edit.
    fn node_mut(
        graph: &mut GraphDefinition,
        uuid: Uuid,
    ) -> Result<&mut crate::graph::NodeInstance, String> {
        graph
            .nodes
            .iter_mut()
            .find(|node| node.uuid == uuid)
            .ok_or_else(|| "no such node in the definition".to_owned())
    }

    /// Set a node's label override, to any name the message carries. An
    /// empty label clears the override — an empty field means "default",
    /// the node returning to its type's label.
    fn set_label(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let label = protocol::take_string(fields, "label")?;
        protocol::done(fields)?;
        Self::node_mut(&mut session.graph, uuid)?.label =
            if label.is_empty() { None } else { Some(label) };
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "label_set" }))
    }

    /// Set an input's parameter value: the typed text the message
    /// carries, read server-side as the file format reads a hand-written
    /// one; an absent value clears, an unset input being unset. A
    /// connected input refuses the edit — an input carries a connection
    /// or a literal, never both. Beyond that nothing is judged here:
    /// whether the name is a port of the type and whether the value
    /// suits it is compile time's business.
    fn set_parameter(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let input = protocol::take_string(fields, "input")?;
        let value = protocol::take_parameter(fields, "value")?;
        protocol::done(fields)?;
        let named = Self::node(&session.graph, uuid)?;
        if session
            .graph
            .edges
            .iter()
            .any(|edge| edge.to == uuid && edge.to_port == input)
        {
            return Err(format!(
                "input `{input}` of node {} receives a connection; a connected input carries no parameter value",
                named.name(self.registry.node_type(&named.type_ref))
            ));
        }
        let node = Self::node_mut(&mut session.graph, uuid)?;
        match value {
            Some(value) => {
                node.parameters.insert(input, value);
            }
            None => {
                node.parameters.remove(&input);
            }
        }
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "parameter_set" }))
    }

    /// Whether the node an operation names is in the definition — the
    /// existence check for operations that name one without editing it.
    fn require_node(graph: &GraphDefinition, uuid: Uuid) -> Result<(), String> {
        Self::node(graph, uuid).map(|_| ())
    }

    /// Wire an output to an input, naming the edge it means. Edit time
    /// judges nothing about the wire — not the ports, not the types, not
    /// cycles; compile time does, when a run starts.
    fn wire(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let from = protocol::take_uuid(fields, "from")?;
        let from_port = protocol::take_string(fields, "from_port")?;
        let to = protocol::take_uuid(fields, "to")?;
        let to_port = protocol::take_string(fields, "to_port")?;
        protocol::done(fields)?;
        let graph = &mut session.graph;
        for uuid in [from, to] {
            Self::require_node(graph, uuid)?;
        }
        // An input carries one value source: the landing wire replaces
        // whatever upstream edge and parameter literal the input held.
        graph
            .edges
            .retain(|edge| edge.to != to || edge.to_port != to_port);
        if let Some(node) = graph.nodes.iter_mut().find(|node| node.uuid == to) {
            node.parameters.remove(&to_port);
        }
        graph.edges.push(Edge {
            from,
            from_port,
            to,
            to_port,
        });
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "wired" }))
    }

    /// Unhook an input: its edge, if any, goes. An input carries at most
    /// one upstream, so the input end names the edge; an input with no
    /// edge changes nothing and answers the same.
    fn unhook(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let to = protocol::take_uuid(fields, "to")?;
        let to_port = protocol::take_string(fields, "to_port")?;
        protocol::done(fields)?;
        Self::require_node(&session.graph, to)?;
        let before = session.graph.edges.len();
        session
            .graph
            .edges
            .retain(|edge| edge.to != to || edge.to_port != to_port);
        if session.graph.edges.len() != before {
            session.dirty = true;
            self.push_definition(session);
        }
        Ok(json!({ "type": "unhooked" }))
    }

    /// Delete a node together with every wire attached to it — no dangling
    /// edges, which the file format forbids.
    fn delete_node(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        protocol::done(fields)?;
        Self::require_node(&session.graph, uuid)?;
        session.graph.nodes.retain(|node| node.uuid != uuid);
        session
            .graph
            .edges
            .retain(|edge| edge.from != uuid && edge.to != uuid);
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "node_deleted" }))
    }

    /// Package the selection the message names; `packaging::package` does
    /// the work and the reply carries the new instance's uuid.
    fn package_group(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let nodes = protocol::take_uuids(fields, "nodes")?;
        let name = protocol::take_string(fields, "name")?;
        protocol::done(fields)?;
        let uuid = packaging::package(&mut session.graph, &nodes, &name, &self.registry)?;
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "group_packaged", "uuid": uuid, "name": name }))
    }

    /// Unpack one group instance back into its nodes; the reply carries
    /// the uuids the nodes returned under, so the client can hand them
    /// the selection the instance held.
    fn unpack_group(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        protocol::done(fields)?;
        let nodes = packaging::unpack(&mut session.graph, uuid, &self.registry)?;
        session.dirty = true;
        self.push_definition(session);
        Ok(json!({ "type": "group_unpacked", "nodes": nodes }))
    }

    /// Open a graph file: the story 03 loader parses it structurally and
    /// the held definition is replaced — loading is structural only, so a
    /// file referencing types this binary never linked opens with
    /// placeholders, nothing judged but the document's shape. The file
    /// becomes the current one and the definition is clean. The reading
    /// happens before the session is touched: a failure is reported
    /// naming the path and what and where, and the held graph and
    /// current file stay as they were.
    fn open_file(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let path = protocol::take_string(fields, "path")?;
        protocol::done(fields)?;
        let graph = read_definition(&path)?;
        session.graph = graph;
        session.file = Some(path);
        session.dirty = false;
        self.push_definition(session);
        Ok(json!({ "type": "file_opened" }))
    }

    /// Save the held definition as YAML through the format's own dump —
    /// exactly what the definition is, schema version and all, nothing
    /// invented at save time. The target is the path the message names —
    /// a first save or a save elsewhere — or the current file; a save with
    /// no target and none held is refused. The whole document is written
    /// or none of it, and only a completed save retargets the current file
    /// and clears the unsaved-changes state.
    fn save_file(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        let asked = protocol::take_optional_string(fields, "path")?;
        protocol::done(fields)?;
        let target = asked.or_else(|| session.file.clone()).ok_or_else(|| {
            "nothing to save to: an untitled graph's first save must name a path".to_owned()
        })?;
        let document = graph::dump(&session.graph);
        write_whole(Path::new(&target), &document)
            .map_err(|error| format!("cannot save to `{target}`: {error}"))?;
        session.file = Some(target);
        session.dirty = false;
        self.push_file(session);
        Ok(json!({ "type": "file_saved" }))
    }

    /// Start a fresh graph: an empty, untitled definition, clean. The same
    /// replace path an open takes, completing the file model.
    fn new_graph(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        protocol::done(fields)?;
        session.graph = GraphDefinition::empty();
        session.file = None;
        session.dirty = false;
        self.push_definition(session);
        Ok(json!({ "type": "graph_created" }))
    }

    /// Start a run: the held definition compiles afresh — every start
    /// compiles, nothing compiled survives a run, so whatever was edited
    /// last is exactly what runs — and a clean compile begins the run, its
    /// state pushed to every connection. The compiled graph moves into the
    /// run's own task and dies with it; the endings come back through the
    /// engine's event stream. The whole operation holds the session lock:
    /// the definition compiled is exactly the last one before running, and
    /// no edit can land between the compile and the run's lock.
    ///
    /// A run already on refuses a second start; an empty definition has
    /// nothing to run and is named; a definition that does not compile is
    /// answered with the errors, which name what and where, the state left
    /// idle and editing untouched — enforcement is compile time's, never
    /// the editor's.
    fn start_run(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        protocol::done(fields)?;
        if session.run.running() {
            return Err("a run is already on: stop it before starting another".to_owned());
        }
        if session.graph.nodes.is_empty() {
            return Err("the held definition has no nodes: there is nothing to run".to_owned());
        }
        let result = compile::compile(&session.graph, &self.registry);
        let Some(compiled) = result.graph else {
            return Err(format!(
                "the definition does not compile:\n{}",
                result
                    .errors
                    .iter()
                    .map(|problem| problem.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        };
        let (stop, stop_requested) = watch::channel(false);
        session.run = RunState::Running { stop };
        session.generation += 1;
        // The next start resets the canvas: the last run's statuses and
        // values give way as this run's own events arrive through the
        // bridge.
        session.display.reset();
        self.push_run(session);
        run::spawn(
            Arc::clone(&self.session),
            self.pushes.clone(),
            Arc::clone(&self.registry),
            compiled,
            stop_requested,
            self.taps.clone(),
            session.generation,
        );
        Ok(json!({ "type": "run_started" }))
    }

    /// Stop the run that is on: the engine ends it — remaining work
    /// stopped, in-flight delivery not promised — and its run-finished
    /// event carries the stopped outcome back to every connection. No run
    /// on, nothing to stop, named.
    fn stop_run(
        &self,
        session: &Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        protocol::done(fields)?;
        match &session.run {
            RunState::Running { stop } => {
                let _ = stop.send(true);
            }
            RunState::Idle { .. } => return Err("no run is on to stop".to_owned()),
        }
        Ok(json!({ "type": "run_stopped" }))
    }

    /// Reset the run: back to the idle canvas. A run still on is asked to
    /// stop, and either way the outcome, the node statuses, and the port
    /// values go, leaving the state a session has before its first run —
    /// a fresh start compiles afresh, as every start does.
    ///
    /// The run the reset ends goes on ending in its own time, and the
    /// generation moves past it here, so its late statuses, its emissions,
    /// and its own run-finished are heard for a run the session is no
    /// longer on and dropped. Nothing to reset — never run, or already
    /// reset — is named; the editor's control is disabled without a run
    /// state, so this answers a stale tab, not a gesture.
    fn reset_run(
        &self,
        session: &mut Session,
        fields: &mut serde_json::Map<String, Value>,
    ) -> Result<Value, String> {
        protocol::done(fields)?;
        match &session.run {
            RunState::Running { stop } => {
                let _ = stop.send(true);
            }
            RunState::Idle { outcome: None } => {
                return Err("no run to reset: none is on and none has run".to_owned())
            }
            RunState::Idle { .. } => {}
        }
        session.generation += 1;
        session.run = RunState::Idle { outcome: None };
        session.display.reset();
        self.push_run(session);
        self.push_display(session);
        Ok(json!({ "type": "run_reset" }))
    }

    /// Push the whole updated definition, with the file state, the run
    /// state, and the problems beside it — never the operation. Every
    /// definition change lands here, so the problems the compile finds are
    /// recomputed beside it: held as state like the definition itself,
    /// travelling with it wherever it travels. The browser holds no graph
    /// state of its own, so a push it can render
    /// without applying or merging anything is the one shape that can
    /// never diverge from what the server holds. Called with the session
    /// still locked, so the pushes leave in the order the operations
    /// applied and an older snapshot can never arrive after a newer one.
    fn push_definition(&self, session: &mut Session) {
        session.problems = problems_of(&session.graph, &self.registry);
        let message = match protocol::definition_message(
            &session.graph,
            session.file.as_deref(),
            session.dirty,
            &session.run,
            &session.problems,
        ) {
            Ok(message) => message,
            Err(error) => protocol::error_reply(
                None,
                &format!("the updated definition cannot be carried as JSON: {error}"),
            ),
        };
        let _ = self.pushes.send(message);
    }

    /// Push the file state alone: the one file change that leaves the
    /// definition untouched, a save.
    fn push_file(&self, session: &Session) {
        let _ = self.pushes.send(protocol::file_message(
            session.file.as_deref(),
            session.dirty,
        ));
    }

    /// Push the run state alone: the one run change that leaves the
    /// definition untouched, a run starting or ending.
    fn push_run(&self, session: &Session) {
        let _ = self.pushes.send(protocol::run_message(&session.run));
    }

    /// Push the canvas's run display alone — the statuses and the held
    /// port values, as a whole: the shape a connecting tab joins with, and
    /// the shape a reset empties.
    fn push_display(&self, session: &Session) {
        let _ = self
            .pushes
            .send(protocol::run_display_message(&session.display));
    }

    async fn accept(self: &Arc<Self>, mut stream: TcpStream) -> io::Result<()> {
        let request = match http::read_request(&mut stream).await {
            Ok(request) => request,
            Err(error) => {
                http::write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    format!("{error}\n").as_bytes(),
                )
                .await?;
                return Ok(());
            }
        };
        if request.method != "GET" {
            return http::write_response(
                &mut stream,
                "405 Method Not Allowed",
                "text/plain; charset=utf-8",
                b"the editor serves GET only\n",
            )
            .await;
        }
        if request.path == "/ws" {
            let Some(key) = request.websocket_key else {
                return http::write_response(
                    &mut stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"the websocket endpoint needs a websocket handshake\n",
                )
                .await;
            };
            http::write_upgrade(&mut stream, &ws::accept_key(&key)).await?;
            return self.connection(stream).await;
        }
        // The embedded page and the plugin-declared UI bundles serve the one
        // way: a declared asset's bytes, or the 404 a missing path answers
        // with — a missing bundle is a browser-reported failure, never a
        // crash.
        match self.asset(&request.path) {
            Some((content_type, body)) => {
                http::write_response(&mut stream, "200 OK", content_type, body.as_bytes()).await
            }
            None => {
                http::write_response(
                    &mut stream,
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    b"not found\n",
                )
                .await
            }
        }
    }

    /// The served file at `path`: the embedded editor UI, or one of the
    /// plugin-declared UI bundles.
    fn asset(&self, path: &str) -> Option<(&'static str, &'static str)> {
        assets::find(path)
            .map(|asset| (asset.content_type, asset.body))
            .or_else(|| {
                self.plugin_assets
                    .iter()
                    .find(|asset| asset.path == path)
                    .map(|asset| (asset.content_type, asset.body))
            })
    }

    /// One connection: a greeting, then every received message answered and
    /// every push delivered, over one socket. Runs until the peer closes or
    /// the transport fails.
    async fn connection(self: &Arc<Self>, stream: TcpStream) -> io::Result<()> {
        let (mut reader, mut writer) = tokio::io::split(stream);
        let (outbound, mut outbound_rx) = mpsc::channel::<Outbound>(64);
        outbound
            .send(Outbound::Text(protocol::greeting()))
            .await
            .expect("the outbound queue is fresh");
        let mut pushes = self.pushes.subscribe();
        let pump = tokio::spawn(async move {
            loop {
                tokio::select! {
                    push = pushes.recv() => match push {
                        Ok(text) => {
                            if ws::write_text(&mut writer, &text).await.is_err() {
                                break;
                            }
                        }
                        // A connection that cannot keep up with the pushes
                        // can no longer trust what it missed; it is dropped,
                        // and a reload or the client's own retry resyncs it
                        // from the held state.
                        Err(broadcast::error::RecvError::Lagged(_)) => break,
                        Err(broadcast::error::RecvError::Closed) => break,
                    },
                    outbound = outbound_rx.recv() => match outbound {
                        Some(Outbound::Text(text)) => {
                            if ws::write_text(&mut writer, &text).await.is_err() {
                                break;
                            }
                        }
                        Some(Outbound::Pong(payload)) => {
                            if ws::write_pong(&mut writer, &payload).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    },
                }
            }
            // However the pump ends — dropped past its bound, the channel
            // closed, a write failed — the connection ends with it: the
            // socket is shut down rather than left open and silent, so the
            // browser sees the loss and rejoins through the connect-time
            // resync.
            let _ = writer.shutdown().await;
        });
        let mut messages = ws::Reader::new();
        while let Some(message) = messages.read(&mut reader).await.unwrap_or(None) {
            match message {
                ws::Message::Text(text) => {
                    if outbound
                        .send(Outbound::Text(self.handle(&text)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                ws::Message::Ping(payload) => {
                    let _ = outbound.send(Outbound::Pong(payload)).await;
                }
                ws::Message::Pong(_) => {}
            }
        }
        pump.abort();
        Ok(())
    }
}

/// The problems the compile finds in `graph`, errors and warnings both:
/// the same compile a start runs, cheap enough to ride every edit, so
/// the held problems are current by construction. No second validator,
/// no check the browser performs.
fn problems_of(graph: &GraphDefinition, registry: &Registry) -> Vec<compile::Problem> {
    let result = compile::compile(graph, registry);
    result.errors.into_iter().chain(result.warnings).collect()
}

/// Read a graph file the way every editor door does — the launch seed
/// and `open` alike: the story 03 loader's structural parse, then the
/// check that the browser's JSON push can carry what it parsed. A
/// hand-written file can hold metadata JSON cannot (a non-finite float
/// key, say), and a definition no tab can render fails here, naming the
/// path and the fault, before any session is touched.
pub fn read_definition(path: &str) -> Result<GraphDefinition, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("cannot read `{path}`: {error}"))?;
    let definition =
        graph::load(&text).map_err(|error| format!("cannot open `{path}`: {error}"))?;
    serde_json::to_value(&definition).map_err(|error| format!("cannot open `{path}`: {error}"))?;
    Ok(definition)
}

/// Write a document so the target ends up holding it whole or keeps its
/// old content: the text lands in a sibling temporary file first and is
/// renamed over the target only once complete, so a failed save never
/// leaves the target half-written. The temporary file is cleaned up on
/// the way out of a failure.
fn write_whole(path: &Path, text: &str) -> io::Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("graph");
    let temp = path.with_file_name(format!(".{name}.{}.tmp", Uuid::new_v4().simple()));
    let written = fs::write(&temp, text).and_then(|()| fs::rename(&temp, path));
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}
