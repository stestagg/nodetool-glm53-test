# Developing nodetool

Nodetool is a Rust workspace for showing, editing, and executing asynchronous
dataflow graphs. The `nodetool` crate is the core: it defines what a node type
is and keeps the registry that gathers node type declarations from every
plugin crate linked into a binary. Core contains no node types of its own.

## Setup

- A stable Rust toolchain, via [rustup](https://rustup.rs).
- A C toolchain for linking (`cc`/gcc).

## Layout

- `crates/nodetool` — the core library: the node type model, the
  [`node_type!`] declaration macro, and the registry.
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

A plugin is an ordinary crate whose only dependency is `nodetool`.

1. Declare node types:

```rust
use nodetool::node_type;

node_type! {
    type_ref: "shapes/circle",     // stable key: registry, graph files, compiler
    label: "Circle",
    icon: r##"<svg ...>...</svg>"##,
    plugin: "shapes",              // the plugin's group
    sub_group: "2d",               // optional
    inputs:  [ radius: ["i32", "f64"] ],
    outputs: [ shape: "shapes/shape" ],
}
```

Each port carries one or more declared type references, written as a single
literal or a bracketed list.

2. Link the plugin into a binary. Declaration is the whole registration step
   — `inventory` collects the declarations of every statically linked plugin
   crate. The one honest exception: a linker discards an rlib that nothing
   references, so a binary (or test) that never otherwise names a plugin
   crate keeps it linked with one anchor line per plugin crate:

```rust
use plugin_shapes as _;
```

3. Read the registry: `nodetool::registry::node_types()` serves every linked
   plugin's declarations; `nodetool::registry::node_type(ref)` looks one up by
   its type reference. Declaring a type reference another plugin already
   declared panics the first time the registry is read, naming the reference
   and both plugins.
