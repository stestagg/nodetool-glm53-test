# Developing nodetool

Nodetool shows, edits, and executes asynchronous dataflow graphs. A graph is
a YAML file of node instances and the edges between them; the `nodetool`
crate compiles one and runs it as live, streaming execution, and serves a
browser editor over the same definition. Node types come from plugin
crates — core ships none — so a binary's palette is whatever it links.

The workspace: `crates/nodetool` is the core library and the editor server,
`crates/nodetool-utility` and `crates/nodetool-fizzbuzz` are the
first-party plugin crates (the latter with a binary that runs a graph
headless or hosts the editor), and `examples/` holds demo binaries and the
plugins they link.

## Setup

- A stable Rust toolchain, via [rustup](https://rustup.rs).
- A C toolchain for linking (`cc`/gcc).
- Node.js with npm, for the editor UI the server embeds; cargo drives that
  build itself, so there is no npm command to run by hand.

## Build and check

The editor UI builds first — the server embeds its output — and cargo
carries that step: every `cargo build`, `cargo test`, and `cargo run`
rebuilds the bundle when a file under `crates/nodetool/ui` has changed, so
the cargo commands below are the whole Rust-side check. The UI's own test
suite is not part of a cargo build; run it too.

```sh
cargo build
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --check
(cd crates/nodetool/ui && npm test)
```

`NODETOOL_SKIP_UI_BUILD=1` leaves an already-built `crates/nodetool/ui/dist`
alone, for building with no Node toolchain at hand. Only the UI's sources
are watched, not its output, so a `dist/` removed by hand is not noticed
until the next UI change — `(cd crates/nodetool/ui && npm run build)` puts
it back.

## Run

```sh
cargo run -p list-nodes        # the node and data type listing two plugins contribute
cargo run -p list-nodes-empty  # the same listing with no plugin linked
cargo run -p load-graph        # load a graph file, print it, show the round trip
cargo run -p compile-graph     # compile each sample graph file, print the result
cargo run -p run-node          # drive one node's behaviour with scripted input streams
cargo run -p run-graph         # run a sample graph headless, values printed as they arrive
cargo run -p visual            # the editor, on the address it prints
cargo run -p visual -- examples/visual/graphs/ticker.yml   # the editor on a graph file
cargo run -p visual-badui      # the editor with a deliberately unknown UI contract linked
cargo run -p nodetool-fizzbuzz -- crates/nodetool-fizzbuzz/graphs/fizzbuzz.yml       # a hundred lines, as they arrive
cargo run -p nodetool-fizzbuzz -- --ui crates/nodetool-fizzbuzz/graphs/fizzbuzz.yml  # the same graph in the editor, still printing here
```

The editor binaries print the address they serve; open it in a browser.
They bind a loopback default, so a second one refuses to start — pass
`--address host:port` to `nodetool-fizzbuzz --ui` to serve elsewhere, port
zero taking whichever port is free.

`run-graph` takes a sample name, and `observe` beside it adds the engine's
event timeline; naming a sample that does not exist lists the ones that do.

## Where the specs live

The rustdoc is the spec — `cargo doc -p nodetool --open`:

- `nodetool::graph` — the versioned YAML graph file format, groups included.
- `nodetool::compile` — how a definition becomes an executable graph.
- `nodetool::behaviour` — the node authoring API, and the stream semantics
  every behaviour programs against.
- `nodetool::engine` — the run lifecycle, its three endings, and the events
  an observer is told.
- `nodetool::server` — the editor server, its websocket protocol, and the
  seams a hosting binary reaches for.
- `nodetool::node_type!` and `nodetool::data_type!` — the plugin author's
  guide: declaring node types and data types, the optional behaviour, UI,
  and validation arms, and what linking a plugin into a binary does.

`crates/nodetool-utility` is the smallest plugin crate written that way,
and `crates/nodetool-fizzbuzz` the reference for port families and
per-instance choices.
