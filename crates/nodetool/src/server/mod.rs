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
//! the same way. One graph state, not two; edits land last-write-wins,
//! with no locking, presence, or merge ceremony, for a local single-user
//! tool. While a run is on, the definition is held still: the UI quiets
//! its editing controls and the server refuses any edit operation that
//! arrives, the one rule covering every editing operation. Starting a run
//! compiles the held definition afresh — every start compiles, nothing
//! compiled survives a run — and the run's endings come back through the
//! engine's event stream. Opening and saving go through the one file
//! format's loader and dump, so a file the headless run takes is the file
//! the editor edits.
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
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, watch};

use crate::compile;
use crate::graph::{self, Edge, GraphDefinition};
use crate::registry::Registry;
use crate::NodeType;
use uuid::Uuid;

mod assets;
mod http;
mod protocol;
mod run;
mod ws;

pub use protocol::{greeting, PROTOCOL_VERSION};
pub use run::{Outcome, RunState};

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
/// changed since that file was last opened or saved, and the run of the
/// graph. One lock over the four, so a file operation cannot interleave
/// with an edit, and a run cannot start between an edit's check and its
/// application.
struct Session {
    graph: GraphDefinition,
    file: Option<String>,
    dirty: bool,
    run: RunState,
}

/// The editing server: the held session, the palette listing of every
/// linked plugin's node types, the registry the start compiles against,
/// and the push channel every connection rides.
pub struct Editor {
    session: Arc<Mutex<Session>>,
    listing: Vec<&'static NodeType>,
    registry: Registry,
    base_scalars: serde_json::Map<String, Value>,
    pushes: broadcast::Sender<String>,
}

/// The message types that edit the held definition — the node operations
/// and the file operations with them. Refused while a run is on; looking
/// (the listing, the definition) stays open.
const EDITING: &[&str] = &[
    "create_node",
    "move_node",
    "set_label",
    "set_parameter",
    "wire",
    "unhook",
    "delete_node",
    "open_file",
    "save_file",
    "new_graph",
];

impl Editor {
    /// An editor holding `definition` and the file it came from, if any —
    /// `None` for a fresh, untitled graph. Launching on a graph file hands
    /// the loader's definition and the file's path here; the definition is
    /// clean until an edit lands, and no run is on.
    pub fn new(definition: GraphDefinition, file: Option<String>) -> Editor {
        let (pushes, _) = broadcast::channel(64);
        Editor {
            session: Arc::new(Mutex::new(Session {
                graph: definition,
                file,
                dirty: false,
                run: RunState::Idle { outcome: None },
            })),
            listing: protocol::node_type_listing(),
            registry: Registry::collect(),
            base_scalars: protocol::base_scalars(),
            pushes,
        }
    }

    /// The session under its one lock: every read and edit holds the
    /// guard for the moment of its operation, the lock never poisoned.
    fn session(&self) -> MutexGuard<'_, Session> {
        self.session
            .lock()
            .expect("the session lock is never poisoned")
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
        // and the file ones with them.
        if EDITING.contains(&kind.as_str()) && self.session().run.running() {
            return protocol::error_reply(
                id,
                "a run is on: the definition is held still until it ends",
            );
        }
        let reply = match kind.as_str() {
            "list_node_types" => self.list_node_types(&mut fields),
            "get_definition" => self.get_definition(&mut fields),
            "create_node" => self.create_node(&mut fields),
            "move_node" => self.move_node(&mut fields),
            "set_label" => self.set_label(&mut fields),
            "set_parameter" => self.set_parameter(&mut fields),
            "wire" => self.wire(&mut fields),
            "unhook" => self.unhook(&mut fields),
            "delete_node" => self.delete_node(&mut fields),
            "open_file" => self.open_file(&mut fields),
            "save_file" => self.save_file(&mut fields),
            "new_graph" => self.new_graph(&mut fields),
            "start_run" => self.start_run(&mut fields),
            "stop_run" => self.stop_run(&mut fields),
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
        }))
    }

    fn get_definition(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        protocol::done(fields)?;
        let session = self.session();
        let graph = serde_json::to_value(&session.graph)
            .map_err(|error| format!("the held definition cannot be carried as JSON: {error}"))?;
        Ok(json!({
            "type": "definition",
            "graph": graph,
            "file": protocol::file_state(session.file.as_deref(), session.dirty),
            "run": protocol::run_state(&session.run),
        }))
    }

    fn create_node(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let type_ref = protocol::take_string(fields, "type_ref")?;
        let position = protocol::take_position(fields, "position")?;
        protocol::done(fields)?;
        if !self
            .listing
            .iter()
            .any(|node_type| node_type.type_ref == type_ref)
        {
            return Err(format!(
                "no node type `{type_ref}` is linked into this binary"
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
        let mut session = self.session();
        session.graph.nodes.push(node);
        session.dirty = true;
        self.push_definition(&session);
        Ok(json!({ "type": "node_created", "uuid": uuid }))
    }

    fn move_node(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let position = protocol::take_position(fields, "position")?;
        protocol::done(fields)?;
        let mut session = self.session();
        // The move records where the node rests; the rest of its metadata
        // is the node's own bookkeeping and stays untouched.
        Self::node_mut(&mut session.graph, uuid)?
            .metadata
            .insert("position".into(), position.metadata_entry().into());
        session.dirty = true;
        self.push_definition(&session);
        Ok(json!({ "type": "node_moved" }))
    }

    /// The node an operation names, required to be in the definition.
    fn node_mut(
        graph: &mut GraphDefinition,
        uuid: Uuid,
    ) -> Result<&mut crate::graph::NodeInstance, String> {
        graph
            .nodes
            .iter_mut()
            .find(|node| node.uuid == uuid)
            .ok_or_else(|| format!("no node {uuid} in the definition"))
    }

    /// Set a node's label override, to any name the message carries. An
    /// empty label clears the override — an empty field means "default",
    /// the node returning to its type's label.
    fn set_label(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let label = protocol::take_string(fields, "label")?;
        protocol::done(fields)?;
        let mut session = self.session();
        Self::node_mut(&mut session.graph, uuid)?.label =
            if label.is_empty() { None } else { Some(label) };
        session.dirty = true;
        self.push_definition(&session);
        Ok(json!({ "type": "label_set" }))
    }

    /// Set an input's parameter value: the typed text the message
    /// carries, read server-side as the file format reads a hand-written
    /// one; an absent value clears, an unset input being unset. A
    /// connected input refuses the edit — an input carries a connection
    /// or a literal, never both. Beyond that nothing is judged here:
    /// whether the name is a port of the type and whether the value
    /// suits it is compile time's business.
    fn set_parameter(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let input = protocol::take_string(fields, "input")?;
        let value = protocol::take_parameter(fields, "value")?;
        protocol::done(fields)?;
        let mut session = self.session();
        if session
            .graph
            .edges
            .iter()
            .any(|edge| edge.to == uuid && edge.to_port == input)
        {
            return Err(format!(
                "input `{input}` of node {uuid} receives a connection; a connected input carries no parameter value"
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
        self.push_definition(&session);
        Ok(json!({ "type": "parameter_set" }))
    }

    /// Whether the node an operation names is in the definition — the
    /// existence check for operations that name one without editing it.
    fn require_node(graph: &GraphDefinition, uuid: Uuid) -> Result<(), String> {
        if graph.nodes.iter().any(|node| node.uuid == uuid) {
            Ok(())
        } else {
            Err(format!("no node {uuid} in the definition"))
        }
    }

    /// Wire an output to an input, naming the edge it means. Edit time
    /// judges nothing about the wire — not the ports, not the types, not
    /// cycles; compile time does, when a run starts.
    fn wire(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let from = protocol::take_uuid(fields, "from")?;
        let from_port = protocol::take_string(fields, "from_port")?;
        let to = protocol::take_uuid(fields, "to")?;
        let to_port = protocol::take_string(fields, "to_port")?;
        protocol::done(fields)?;
        let mut session = self.session();
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
        self.push_definition(&session);
        Ok(json!({ "type": "wired" }))
    }

    /// Unhook an input: its edge, if any, goes. An input carries at most
    /// one upstream, so the input end names the edge; an input with no
    /// edge changes nothing and answers the same.
    fn unhook(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let to = protocol::take_uuid(fields, "to")?;
        let to_port = protocol::take_string(fields, "to_port")?;
        protocol::done(fields)?;
        let mut session = self.session();
        Self::require_node(&session.graph, to)?;
        let before = session.graph.edges.len();
        session
            .graph
            .edges
            .retain(|edge| edge.to != to || edge.to_port != to_port);
        if session.graph.edges.len() != before {
            session.dirty = true;
            self.push_definition(&session);
        }
        Ok(json!({ "type": "unhooked" }))
    }

    /// Delete a node together with every wire attached to it — no dangling
    /// edges, which the file format forbids.
    fn delete_node(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        protocol::done(fields)?;
        let mut session = self.session();
        Self::require_node(&session.graph, uuid)?;
        session.graph.nodes.retain(|node| node.uuid != uuid);
        session
            .graph
            .edges
            .retain(|edge| edge.from != uuid && edge.to != uuid);
        session.dirty = true;
        self.push_definition(&session);
        Ok(json!({ "type": "node_deleted" }))
    }

    /// Open a graph file: the story 03 loader parses it structurally and
    /// the held definition is replaced — loading is structural only, so a
    /// file referencing types this binary never linked opens with
    /// placeholders, nothing judged but the document's shape. The file
    /// becomes the current one and the definition is clean. The reading
    /// happens before the session is touched: a failure is reported
    /// naming the path and what and where, and the held graph and
    /// current file stay as they were.
    fn open_file(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let path = protocol::take_string(fields, "path")?;
        protocol::done(fields)?;
        let graph = read_definition(&path)?;
        let mut session = self.session();
        session.graph = graph;
        session.file = Some(path);
        session.dirty = false;
        self.push_definition(&session);
        Ok(json!({ "type": "file_opened" }))
    }

    /// Save the held definition as YAML through the format's own dump —
    /// exactly what the definition is, schema version and all, nothing
    /// invented at save time. The target is the path the message names —
    /// a first save or a save elsewhere — or the current file; a save with
    /// no target and none held is refused. The whole document is written
    /// or none of it, and only a completed save retargets the current file
    /// and clears the unsaved-changes state.
    fn save_file(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let asked = protocol::take_optional_string(fields, "path")?;
        protocol::done(fields)?;
        let mut session = self.session();
        let target = asked.or_else(|| session.file.clone()).ok_or_else(|| {
            "nothing to save to: an untitled graph's first save must name a path".to_owned()
        })?;
        let document = graph::dump(&session.graph);
        write_whole(Path::new(&target), &document)
            .map_err(|error| format!("cannot save to `{target}`: {error}"))?;
        session.file = Some(target);
        session.dirty = false;
        self.push_file(&session);
        Ok(json!({ "type": "file_saved" }))
    }

    /// Start a fresh graph: an empty, untitled definition, clean. The same
    /// replace path an open takes, completing the file model.
    fn new_graph(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        protocol::done(fields)?;
        let mut session = self.session();
        session.graph = GraphDefinition::empty();
        session.file = None;
        session.dirty = false;
        self.push_definition(&session);
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
    fn start_run(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        protocol::done(fields)?;
        let mut session = self.session();
        if session.run.running() {
            return Err("a run is already on: stop it before starting another".to_owned());
        }
        if session.graph.nodes.is_empty() {
            return Err("the held definition has no nodes: there is nothing to run".to_owned());
        }
        let compiled = compile::compile(&session.graph, &self.registry)
            .map_err(|errors| format!("the definition does not compile:\n{}", errors.join("\n")))?;
        let (stop, stop_requested) = watch::channel(false);
        session.run = RunState::Running { stop };
        self.push_run(&session);
        run::spawn(
            Arc::clone(&self.session),
            self.pushes.clone(),
            compiled,
            stop_requested,
        );
        Ok(json!({ "type": "run_started" }))
    }

    /// Stop the run that is on: the engine ends it — remaining work
    /// stopped, in-flight delivery not promised — and its run-finished
    /// event carries the stopped outcome back to every connection. No run
    /// on, nothing to stop, named.
    fn stop_run(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        protocol::done(fields)?;
        let session = self.session();
        match &session.run {
            RunState::Running { stop } => {
                let _ = stop.send(true);
            }
            RunState::Idle { .. } => return Err("no run is on to stop".to_owned()),
        }
        Ok(json!({ "type": "run_stopped" }))
    }

    /// Push the whole updated definition, with the file state and the run
    /// state beside it, to every connection — never the operation. The
    /// browser holds no graph state of its own, so a push it can render
    /// without applying or merging anything is the one shape that can
    /// never diverge from what the server holds. Called with the session
    /// still locked, so the pushes leave in the order the operations
    /// applied and an older snapshot can never arrive after a newer one.
    fn push_definition(&self, session: &Session) {
        let message = match protocol::definition_message(
            &session.graph,
            session.file.as_deref(),
            session.dirty,
            &session.run,
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
        match assets::find(&request.path) {
            Some(asset) => {
                http::write_response(
                    &mut stream,
                    "200 OK",
                    asset.content_type,
                    asset.body.as_bytes(),
                )
                .await
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
                        // can no longer trust what it missed; it ends, and
                        // a reload resyncs from the held definition.
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
