//! The editor UI, embedded at build time: the Vite build of
//! `crates/nodetool/ui` is included here and served by the HTTP server, so
//! one address carries the page and its websocket endpoint alike. Build the
//! UI (see DEVELOPMENT.md) before building this crate. The plugin-declared
//! UI bundles travel the same seam — declared through the registry, served
//! under a per-plugin path.

use std::collections::HashMap;

/// One served file: its path, content type, and body.
pub struct Asset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub body: &'static str,
}

const ASSETS: &[Asset] = &[
    Asset {
        path: "/",
        content_type: "text/html; charset=utf-8",
        body: include_str!("../../ui/dist/index.html"),
    },
    Asset {
        path: "/editor.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_str!("../../ui/dist/editor.js"),
    },
    Asset {
        path: "/editor.css",
        content_type: "text/css; charset=utf-8",
        body: include_str!("../../ui/dist/editor.css"),
    },
];

/// The asset served at `path`, if there is one.
pub fn find(path: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|asset| asset.path == path)
}

/// One plugin-declared UI bundle: the entry asset's served path, its
/// content type, and the body the declaring crate embedded.
pub struct PluginAsset {
    /// The served path, `/plugins/<entry>`.
    pub path: String,
    pub content_type: &'static str,
    pub body: &'static str,
}

/// The per-plugin path every declared bundle serves under — the one path
/// scheme, composed here where the declarations are read, so the browser
/// hardcodes no plugin, type, or asset path.
const PLUGIN_PREFIX: &str = "/plugins/";

/// Every plugin-declared UI bundle of the linked plugins: node-type UI and
/// type-value UI alike, served the one way. Two declarations naming the
/// same entry is a conflict no server can answer honestly — reported here,
/// naming both declarations, the way the registry reports taken names.
pub fn plugin_assets() -> Vec<PluginAsset> {
    let mut assets: Vec<PluginAsset> = Vec::new();
    let mut seen = HashMap::<String, String>::new();
    let mut declare = |entry: &'static str, source: &'static str, declared: &str| {
        let path = format!("{PLUGIN_PREFIX}{entry}");
        if let Some(first) = seen.get(&path) {
            panic!("duplicate plugin UI asset `{path}`: named by both {first} and {declared}");
        }
        seen.insert(path.clone(), declared.to_owned());
        assets.push(PluginAsset {
            path,
            content_type: content_type(entry),
            body: source,
        });
    };
    for node_type in crate::registry::node_types() {
        if let Some(ui) = node_type.ui {
            declare(
                ui.entry,
                ui.source,
                &format!("node type {}", node_type.type_ref),
            );
        }
    }
    for data_type in crate::registry::data_types() {
        if let Some(ui) = data_type.ui {
            declare(
                ui.entry,
                ui.source,
                &format!("data type {}", data_type.name),
            );
        }
    }
    assets
}

/// The content type an entry serves as, read off its extension — a bundle
/// is JavaScript today; the other web shapes come free.
fn content_type(entry: &str) -> &'static str {
    match entry.rsplit('.').next().unwrap_or(entry) {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
