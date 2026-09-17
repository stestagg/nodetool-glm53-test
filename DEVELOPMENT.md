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
  macros, the versioned YAML graph file format (`nodetool::graph`; its
  rustdoc is the format's spec), the compiler that turns a graph definition
  into a compiled, executable graph (`nodetool::compile`), the node
  authoring API and the stream semantics behaviour programs against
  (`nodetool::behaviour`; its rustdoc is the semantics' spec), the engine
  that runs a compiled graph as live, streaming execution and tells an
  optional events observer what happens as the run unfolds
  (`nodetool::engine`; its rustdoc is the run lifecycle's spec), the
  runtime value (`nodetool::Value`), and the registry.
- `crates/nodetool/tests/plugins` — plugin crates that exist for the tests
  (`alpha` is sub-grouped, `beta` is flat, `gamma` supplies the compiler
  tests' node types, `delta` supplies the engine tests' node types).
- `examples/plugins` — small example plugin crates (`shapes` is sub-grouped,
  `text` is flat).
- `examples/list-nodes` — demo binary linked against both example plugins;
  prints the registry listing: node types, then data types with their
  metadata and declared conversions.
- `examples/list-nodes-empty` — the same listing with no plugin linked; core
  ships no node types, so that section is empty, while the base scalars show
  with nothing contributed.
- `examples/load-graph` — demo binary linked against both example plugins;
  loads a graph file and shows the listing, the round trip, and a load
  error; the sample files live in `examples/load-graph/graphs/`.
- `examples/compile-graph` — demo binary linked against both example
  plugins; loads each sample graph definition in
  `examples/compile-graph/graphs/`, compiles it, and prints the compiled
  graph or the compile errors.
- `examples/run-node` — demo binary linked against both example plugins;
  runs one node of the text plugin — its behaviour declared through the
  authoring API — with two scripted input streams, one of them a single
  value that completes, printing the arrival that fired each run, each
  emitted value as it arrives, and the node's completion.
- `examples/run-graph` — demo binary linked against both example plugins;
  the headless path from a terminal. It loads one of the sample graph
  files in `examples/run-graph/graphs/`, compiles it, and runs it with a
  consumer attached to a node's output as one more downstream, printing
  each value as it arrives and then the run's outcome; the `observe`
  flag adds the engine's event timeline beside them. The `pipeline`
  sample completes; the `failing` sample carries a node whose behaviour
  errors mid-run, so the remaining output stops arriving and the error —
  naming the node — is the last word before a non-zero exit; the
  `broken` and `uncompilable` samples show a load error and compile
  errors ending the path, printed, with a non-zero exit.

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
cargo run -p load-graph        # load a graph file, print it, show the round trip
cargo run -p compile-graph     # compile each sample graph file, print the result
cargo run -p run-node          # drive one behaviour-ful node with scripted streams
cargo run -p run-graph         # run the pipeline sample headless, values as they arrive
```

The run-graph samples beyond the default select the demonstration:

```sh
cargo run -p run-graph failing      # fail-fast: the output stops arriving, the error names the node, exit 1
cargo run -p run-graph broken       # a load error ends the path, printed, exit 1
cargo run -p run-graph uncompilable # compile errors end the path, printed, exit 1
```

Adding the `observe` flag subscribes the printing observer, so the
engine's event timeline prints beside whatever the sample shows — the run
itself is the same either way:

```sh
cargo run -p run-graph observe           # the pipeline's event timeline beside the values
cargo run -p run-graph failing observe   # the failing run told line by line, then the same error, exit 1
```

## Writing a plugin

A plugin is an ordinary crate whose only dependency is `nodetool`. Declaring
node types with `nodetool::node_type!` and linking the plugin into a binary is
the whole registration step. The macro's rustdoc is the guide — `cargo doc -p
nodetool --open` — to the declaration form and its linker caveat. A node type
declared with the optional `behaviour` arm runs: the named function builds a
fresh behaviour per instance, implementing `nodetool::behaviour::Behaviour`
against the input and output faces whose stream semantics that module's
rustdoc settles. A type declared without `behaviour` is a descriptor alone.

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
