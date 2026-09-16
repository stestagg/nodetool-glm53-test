# Developing nodetool

Nodetool is a Rust workspace for showing, editing, and executing asynchronous
dataflow graphs. The `nodetool` crate is the core: it defines what a node type
is and what a data type is, and keeps the registry that gathers node type and
data type declarations from every plugin crate linked into a binary. Core
contains no node types and no custom data types of its own — it ships the base
scalar data types.

## Setup

- A stable Rust toolchain, via [rustup](https://rustup.rs).
- A C toolchain for linking (`cc`/gcc).

## Layout

- `crates/nodetool` — the core library: the node type model, the data type
  model with the base scalars, the `node_type!` and `data_type!` declaration
  macros, and the registry.
- `crates/nodetool/tests/plugins` — plugin crates that exist for the registry
  tests (`alpha` is sub-grouped, `beta` is flat).
- `examples/plugins` — small example plugin crates (`shapes` is sub-grouped,
  `text` is flat).
- `examples/list-nodes` — demo binary linked against both example plugins;
  prints the registry listing: node types, then data types with their
  metadata and declared conversions.
- `examples/list-nodes-empty` — the same listing with no plugin linked; core
  ships no node types, so that section is empty, while the base scalars show
  with nothing contributed.

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
cargo run -p list-nodes-empty  # the same listing, no plugin linked
```

## Writing a plugin

A plugin is an ordinary crate whose only dependency is `nodetool`. Declaring
node types with `nodetool::node_type!` and linking the plugin into a binary is
the whole registration step. The macro's rustdoc is the guide — `cargo doc -p
nodetool --open` — to the declaration form and its linker caveat.

Data types are the vocabulary ports refer to. Core ships the base scalars and
declares their trivial conversions through the same mechanism a plugin uses;
nothing converts to `String` automatically. A plugin declares custom types
with `nodetool::data_type!` — its rustdoc is the declaration guide, like
`node_type!`'s above. A conversion may be declared before its target type is
registered; targets resolve once every linked crate has contributed.

Read the node registry with `nodetool::registry::node_types()`, or look one
node type up by its type reference with `nodetool::registry::node_type(type_ref)`.
Declaring a type reference another plugin already declared panics the first
time the registry is read, naming the reference and both plugins.

Read the data type vocabulary with `nodetool::registry::data_types()`, or look
one type up by name with `nodetool::registry::data_type(name)` or by id with
`nodetool::registry::data_type_by_id(id)`. A taken uuid or name, or a
conversion whose target never registers, panics the first time the registry is
read.
