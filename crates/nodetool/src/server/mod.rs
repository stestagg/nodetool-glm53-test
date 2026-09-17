//! The server side of the editor: a small HTTP server that serves the
//! embedded UI over one address and speaks the JSON envelope protocol over
//! one websocket connection per browser.
//!
//! The server owns the graph. It holds the in-memory definition — the same
//! document the files carry and the compiler takes — and the browser is a
//! synced view of it: every edit is an operation applied here, and the
//! whole updated definition is pushed to every connection, so a reload or
//! a second tab simply asks for the definition again. One graph state, not
//! two; edits land last-write-wins, with no locking, presence, or merge
//! ceremony, for a local single-user tool.
//!
//! The websocket is untrusted input at a parse boundary: a message that
//! fails to parse, is not a JSON object, names an unknown type, or carries
//! unknown fields is answered with an error naming the problem, the
//! connection stays open, and nothing can crash or corrupt the server.

use std::io;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};

use crate::graph::{Edge, GraphDefinition};
use crate::NodeType;

mod assets;
mod http;
mod protocol;
mod ws;

pub use protocol::{greeting, PROTOCOL_VERSION};

/// The address the editor serves on by default: loopback only — this is a
/// local tool.
pub const DEFAULT_ADDRESS: &str = "127.0.0.1:8420";

/// One message queued for a connection's write half.
enum Outbound {
    Text(String),
    Pong(Vec<u8>),
}

/// The editing server: the held definition, the palette listing of every
/// linked plugin's node types, and the push channel every connection rides.
pub struct Editor {
    graph: Mutex<GraphDefinition>,
    listing: Vec<&'static NodeType>,
    pushes: broadcast::Sender<String>,
}

impl Editor {
    /// An editor holding `definition` — empty for a fresh graph — and
    /// listing the node types the linked plugins contribute.
    pub fn new(definition: GraphDefinition) -> Editor {
        let (pushes, _) = broadcast::channel(64);
        Editor {
            graph: Mutex::new(definition),
            listing: protocol::node_type_listing(),
            pushes,
        }
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
        let reply = match kind.as_str() {
            "list_node_types" => self.list_node_types(&mut fields),
            "get_definition" => self.get_definition(&mut fields),
            "create_node" => self.create_node(&mut fields),
            "move_node" => self.move_node(&mut fields),
            "wire" => self.wire(&mut fields),
            "unhook" => self.unhook(&mut fields),
            "delete_node" => self.delete_node(&mut fields),
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
        Ok(json!({ "type": "node_types", "node_types": listing }))
    }

    fn get_definition(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        protocol::done(fields)?;
        let graph = self
            .graph
            .lock()
            .expect("the graph lock is never poisoned")
            .clone();
        match serde_json::to_value(&graph) {
            Ok(graph) => Ok(json!({ "type": "definition", "graph": graph })),
            Err(error) => Err(format!(
                "the held definition cannot be carried as JSON: {error}"
            )),
        }
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
        let mut graph = self.graph.lock().expect("the graph lock is never poisoned");
        graph.nodes.push(node);
        self.push_definition(&graph);
        Ok(json!({ "type": "node_created", "uuid": uuid }))
    }

    fn move_node(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        let position = protocol::take_position(fields, "position")?;
        protocol::done(fields)?;
        let mut graph = self.graph.lock().expect("the graph lock is never poisoned");
        let node = graph
            .nodes
            .iter_mut()
            .find(|node| node.uuid == uuid)
            .ok_or_else(|| format!("no node {uuid} in the definition"))?;
        // The move records where the node rests; the rest of its metadata
        // is the node's own bookkeeping and stays untouched.
        node.metadata
            .insert("position".into(), position.metadata_entry().into());
        self.push_definition(&graph);
        Ok(json!({ "type": "node_moved" }))
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
        let mut graph = self.graph.lock().expect("the graph lock is never poisoned");
        for uuid in [from, to] {
            if !graph.nodes.iter().any(|node| node.uuid == uuid) {
                return Err(format!("no node {uuid} in the definition"));
            }
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
        self.push_definition(&graph);
        Ok(json!({ "type": "wired" }))
    }

    /// Unhook an input: its edge, if any, goes. An input carries at most
    /// one upstream, so the input end names the edge; no edge is no
    /// operation, and the reply is the same.
    fn unhook(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let to = protocol::take_uuid(fields, "to")?;
        let to_port = protocol::take_string(fields, "to_port")?;
        protocol::done(fields)?;
        let mut graph = self.graph.lock().expect("the graph lock is never poisoned");
        if !graph.nodes.iter().any(|node| node.uuid == to) {
            return Err(format!("no node {to} in the definition"));
        }
        graph
            .edges
            .retain(|edge| edge.to != to || edge.to_port != to_port);
        self.push_definition(&graph);
        Ok(json!({ "type": "unhooked" }))
    }

    /// Delete a node together with every wire attached to it — no dangling
    /// edges, which the file format forbids.
    fn delete_node(&self, fields: &mut serde_json::Map<String, Value>) -> Result<Value, String> {
        let uuid = protocol::take_uuid(fields, "uuid")?;
        protocol::done(fields)?;
        let mut graph = self.graph.lock().expect("the graph lock is never poisoned");
        if !graph.nodes.iter().any(|node| node.uuid == uuid) {
            return Err(format!("no node {uuid} in the definition"));
        }
        graph.nodes.retain(|node| node.uuid != uuid);
        graph
            .edges
            .retain(|edge| edge.from != uuid && edge.to != uuid);
        self.push_definition(&graph);
        Ok(json!({ "type": "node_deleted" }))
    }

    /// Push the whole updated definition to every connection — never the
    /// operation. The browser holds no graph state of its own, so a push it
    /// can render without applying or merging anything is the one shape
    /// that can never diverge from what the server holds. Called with the
    /// graph still locked, so the pushes leave in the order the operations
    /// applied and an older snapshot can never arrive after a newer one.
    fn push_definition(&self, graph: &GraphDefinition) {
        let message = match protocol::definition_message(graph) {
            Ok(message) => message,
            Err(error) => protocol::error_reply(
                None,
                &format!("the updated definition cannot be carried as JSON: {error}"),
            ),
        };
        let _ = self.pushes.send(message);
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
