//! The editor UI's npm build, carried by cargo: the server embeds the Vite
//! output of `crates/nodetool/ui` (see `src/server/assets.rs`), so the crate
//! cannot compile without it. This script runs that build, which makes
//! `cargo build`, `cargo test`, and `cargo run` one step each — no npm
//! command to remember, and no stale bundle behind a source change.
//!
//! The watched inputs below are the UI's own sources; `ui/dist` is this
//! script's output and is deliberately not watched, so a build does not
//! re-trigger itself. `NODETOOL_SKIP_UI_BUILD=1` leaves an already-built
//! `ui/dist` alone, for a build with no Node toolchain at hand.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let ui = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("ui");

    // Every input the bundle is built from, and nothing else: cargo re-runs
    // this script when one of them changes.
    for input in [
        "src",
        "index.html",
        "package.json",
        "package-lock.json",
        "vite.config.js",
    ] {
        println!("cargo:rerun-if-changed={}", ui.join(input).display());
    }
    println!("cargo:rerun-if-env-changed=NODETOOL_SKIP_UI_BUILD");

    let dist = ui.join("dist");
    if skipping() {
        if !dist.join("editor.js").exists() {
            panic!(
                "NODETOOL_SKIP_UI_BUILD is set but {} holds no build; unset it, or build the UI \
                 by hand (see DEVELOPMENT.md)",
                dist.display()
            );
        }
        return;
    }

    if install_needed(&ui) {
        npm(&ui, &["ci"]);
    }
    npm(&ui, &["run", "build"]);
}

/// Whether the caller asked for the existing `ui/dist` to stand as it is.
fn skipping() -> bool {
    matches!(
        std::env::var("NODETOOL_SKIP_UI_BUILD").as_deref(),
        Ok("1") | Ok("true")
    )
}

/// Whether the dependencies need installing: either there is no install, or
/// the lockfile has moved since the one npm recorded under `node_modules`.
fn install_needed(ui: &Path) -> bool {
    let installed = ui.join("node_modules/.package-lock.json");
    let Ok(installed) = installed.metadata().and_then(|meta| meta.modified()) else {
        return true;
    };
    match ui.join("package-lock.json").metadata() {
        Ok(meta) => meta.modified().map(|lock| lock > installed).unwrap_or(true),
        Err(_) => false,
    }
}

/// Run one npm command in the UI directory, failing the build — with npm's
/// own output, which cargo prints — when it does not succeed.
fn npm(ui: &Path, args: &[&str]) {
    let status = Command::new("npm").args(args).current_dir(ui).status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!(
            "`npm {}` failed in {} ({status})",
            args.join(" "),
            ui.display()
        ),
        Err(error) => panic!(
            "could not run `npm {}` in {}: {error}; Node.js with npm is part of the setup \
             (see DEVELOPMENT.md), or set NODETOOL_SKIP_UI_BUILD=1 to keep an existing build",
            args.join(" "),
            ui.display()
        ),
    }
}
