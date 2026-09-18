//! The setup the editor server's test files share: editors to test
//! against, the one-message seam, and the operation and file helpers the
//! editing and run tests both build from. Each test binary includes the
//! module and uses the slice of it it needs; a helper a binary skips
//! carries its own allow, so one every binary has dropped is still heard.

use std::fs;
use std::path::{Path, PathBuf};

use nodetool::graph;
use nodetool::server::Editor;
use serde_json::Value;

pub fn editor() -> Editor {
    Editor::new(graph::GraphDefinition::empty(), None)
}

pub fn editor_holding(text: &str) -> Editor {
    Editor::new(
        graph::load(text).expect("the test seeds a loadable definition"),
        None,
    )
}

pub fn send(editor: &Editor, message: &str) -> Value {
    serde_json::from_str(&editor.handle(message)).expect("every reply is a JSON object")
}

/// The definition the editor holds, as the connect-time resync carries it.
/// Unused by the run tests, which read the run state off the same reply.
#[allow(dead_code)]
pub fn held_definition(editor: &Editor) -> Value {
    send(editor, r#"{"id": 0, "type": "get_definition"}"#)["graph"].clone()
}

/// Create one node at a fixed spot, answering its uuid: the setup step
/// the wire, label, parameter, unhook, delete, and run tests build
/// graphs from, positions being incidental to them.
pub fn create(editor: &Editor, id: u64, type_ref: &str) -> String {
    send(
        editor,
        &format!(
            r#"{{"id": {id}, "type": "create_node", "type_ref": "{type_ref}", "position": {{"x": 0, "y": 0}}}}"#
        ),
    )["uuid"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Set or clear a node's label override; the empty label means the type's
/// default. Unused by the problems tests, which never rename.
#[allow(dead_code)]
pub fn edit_label(editor: &Editor, id: u64, uuid: &str, label: &str) -> Value {
    send(
        editor,
        &format!(r#"{{"id": {id}, "type": "set_label", "uuid": "{uuid}", "label": "{label}"}}"#),
    )
}

/// Set or clear an input's parameter value. The value is the typed text,
/// carried as a JSON string and read server-side as the file format
/// reads it; `None` commits the empty field, meaning unset. Unused by
/// the run tests, which send their parameter refusals raw.
#[allow(dead_code)]
pub fn edit_parameter(
    editor: &Editor,
    id: u64,
    uuid: &str,
    input: &str,
    value: Option<&str>,
) -> Value {
    let value = value
        .map(|text| format!(r#", "value": {}"#, serde_json::to_string(text).unwrap()))
        .unwrap_or_default();
    send(
        editor,
        &format!(
            r#"{{"id": {id}, "type": "set_parameter", "uuid": "{uuid}", "input": "{input}"{value}}}"#
        ),
    )
}

/// A unique path under the system temp directory, so test runs never
/// collide; the file itself the test writes and removes.
pub fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("nodetool-{name}-{}.yml", uuid::Uuid::new_v4()))
}

/// A graph file on disk carrying `text`: what open and save need.
pub fn written_graph(name: &str, text: &str) -> PathBuf {
    let path = temp_path(name);
    fs::write(&path, text).expect("the test writes its graph file");
    path
}

pub fn path_field(path: &Path) -> String {
    serde_json::to_string(&path.to_string_lossy().into_owned()).unwrap()
}
