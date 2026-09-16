# Developing nodetool

Nodetool is a Rust workspace for showing, editing, and executing asynchronous
dataflow graphs. The `nodetool` crate is the core: it defines what a node type
is and keeps the registry that gathers node type declarations from every
plugin crate linked into a binary. Core contains no node types of its own.

## Setup

- A stable Rust toolchain, via [rustup](https://rustup.rs).
- A C toolchain for linking (`cc`/gcc).

## Layout

- `crates/nodetool` — the core library: the node type model, the `node_type!`
  declaration macro, and the registry.
- `crates/nodetool/tests/plugins` — plugin crates that exist for the registry
  tests (`alpha` is sub-grouped, `beta` is flat).
- `examples/plugins` — small example plugin crates (`shapes` is sub-grouped,
  `text` is flat).
- `examples/list-nodes` — demo binary linked against both example plugins;
  prints the registry listing.
- `examples/list-nodes-empty` — the same listing with no plugin linked; with
  core contributing no node types, it prints it empty.

## Build and check

```sh
cargo build
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --check
```

## Run the examples

```sh
cargo run -p list-nodes        # the listing across both example plugins
cargo run -p list-nodes-empty  # the same listing, empty
```

## Writing a plugin

A plugin is an ordinary crate whose only dependency is `nodetool`. Declaring
node types with `nodetool::node_type!` and linking the plugin into a binary is
the whole registration step. The macro's rustdoc is the guide — `cargo doc -p
nodetool --open` — to the declaration form and its linker caveat.

Read the registry with `nodetool::registry::node_types()`, or look one node
type up by its type reference with `nodetool::registry::node_type(type_ref)`.
Declaring a type reference another plugin already declared panics the first
time the registry is read, naming the reference and both plugins.
