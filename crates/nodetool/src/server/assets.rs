//! The editor UI, embedded at build time: the Vite build of
//! `crates/nodetool/ui` is included here and served by the HTTP server, so
//! one address carries the page and its websocket endpoint alike. Build the
//! UI (see DEVELOPMENT.md) before building this crate.

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
