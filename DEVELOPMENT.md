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
- Node.js with npm, for building the editor UI the server embeds.

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
  runtime value (`nodetool::Value`), the registry, and the editor server
  (`nodetool::server`): a small HTTP server that serves the embedded UI
  over one address and speaks a JSON envelope protocol over one websocket
  connection per browser, holding the graph definition and the file being
  edited as the one authoritative state. Core contains no node types: a
  node library is always a plugin crate, first-party or not.
- `crates/nodetool/ui` — the editor UI the server embeds and serves: a
  React application whose canvas is React Flow under a light Blueprint
  look, with the palette of node types docked on the left and the editing
  sidebar of the selected node on the right. `npm install && npm run
  build` in that directory produces `dist/`, which the server's binary
  includes at build time.
- `crates/nodetool-utility` — the first-party utility node library, a plugin
  crate whose only dependency is `nodetool`: an `If` router and a `Format`
  node, declared through the same `node_type!` registration path and
  authoring API as any third-party plugin. Linking the crate into a binary
  is the whole integration step.
- `crates/nodetool-fizzbuzz` — the first-party fizzbuzz node library, a
  plugin crate like the utility one: a `Counter` source, the seven
  `Condition` comparisons (one node type per operation, under a condition
  sub-group — the six comparisons beside the `Divisible` divisibility
  test), the `Case selection` that pairs a count with its two divisibility
  streams and emits each count's fizzbuzz string, and an `Output`
  terminus. It is the reference for the generic-port idiom — one node type
  per node, its numeric ports declared as one port family (the base
  numeric set) that the compiler resolves to a single concrete type per
  instance, with the numeric logic written once and stamped per resolved
  type (`src/numeric.rs` is the helper). Its binary is the headless
  runner: given a graph file path it loads, compiles, and runs the graph,
  and every output the graph leaves unconnected prints to the terminal as
  it arrives — one line per value, in its plain string form. The shipped
  graph is `graphs/fizzbuzz.yml`, the classic fizzbuzz.
- `crates/nodetool/tests/plugins` — plugin crates that exist for the tests
  (`alpha` is sub-grouped, `beta` is flat, `gamma` supplies the compiler
  tests' node types, `delta` supplies the engine tests' node types).
- `examples/plugins` — small example plugin crates (`shapes` is sub-grouped,
  `text` is flat). The text plugin's `Ticker` is a slow source: fired by
  its `count` parameter it counts from one through the count, a fixed
  pause between values — the one shipped node a run lasts long enough to
  watch, edit against, and stop.
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
- `examples/run-graph` — demo binary linked against both example plugins and
  the utility crate; the headless path from a terminal. It loads one of the
  sample graph files in `examples/run-graph/graphs/`, compiles it, and runs
  it with a consumer attached to a node's output as one more downstream,
  printing each value as it arrives and then the run's outcome; the
  `observe` flag adds the engine's event timeline beside them. The
  `pipeline` sample completes; the `failing` sample carries a node whose
  behaviour errors mid-run, so the remaining output stops arriving and the
  error — naming the node — is the last word before a non-zero exit; the
  `broken` and `uncompilable` samples show a load error and compile
  errors ending the path, printed, with a non-zero exit; the `if-true` and
  `if-false` samples run the same If-and-Format graph under opposite
  conditions, showing the router steering between its two branches, one
  Format with a format template and one without.

## Build and check

The editor UI builds first; the server embeds its output, so the Rust
workspace does not build without it:

```sh
(cd crates/nodetool/ui && npm install && npm run build && npm test)
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
cargo run -p nodetool-fizzbuzz -- crates/nodetool-fizzbuzz/graphs/fizzbuzz.yml
                             # the fizzbuzz run: one hundred lines, as they arrive
cargo run -p visual            # the editor on http://127.0.0.1:8420
cargo run -p visual -- examples/visual/graphs/sample.yml  # the editor on a graph file
```

`visual` starts the editor server: open the printed address in a browser
and find the palette of every node type the linked plugins contribute on
the left and the canvas beside it. Dragging a type onto the canvas creates
a node where it dropped; dragging a node moves it; a click selects,
opening the editing sidebar on the right — the node's label editable to
any name, an empty field returning the type's default, the type reference
and uuid read-only beneath, and below them a field per scalar-possible
input; the same values show as small editable fields on the nodes
themselves, an edit in either appearing in the other, committing on Enter
or on leaving the field. A wire over an input replaces its field with the
declared types in both views; unhooking returns it empty, a replaced
literal not remembered. A committed value is stored as the plain scalar
its text reads as — boolean, integer, float, else string — and a connected
input takes no value. Background drag pans; scroll zooms. Dragging between
an output port and an input port — from either end — wires them; one input
takes at most one upstream, so a wire dropped on an already-wired input
replaces the old wire, and a wire released anywhere it cannot land cancels
quietly. Dragging a wired input's end off and letting go unhooks it.
Delete (or Backspace) removes the selected node together with its wires.
Nothing is checked while editing — types, ports, and cycles are judged
when a run is started. The server holds the graph: a reload or a second
tab shows the same graph, and an edit in one appears in the other.

The chrome carries the run beside the file controls: one control that
reads Start when the editor is idle and Stop while a run is on — the
editor session itself is the interactive mode, there is no mode to
switch on. Start hands the definition the server holds to the compiler —
every start compiles afresh, so whatever was edited last is exactly what
runs — and on a clean compile the run begins: the control flips to Stop
and every connection is pushed the new run state. A compile failure is
reported in the status line, the errors naming what and where; the run
never starts, the state stays idle, and editing stays exactly as free as
it was — the editor never polices, compile decides. While a run is on
the definition is held still: palette drops, node moves, wires and
unhooking, deletion, label and parameter edits, and the file controls
are inert, and a second tab's edit is refused server-side — selection,
panning, and zoom stay live, looking not being editing. The run ends by
itself when every node completes — idle returns, the outcome shows
completed, and editing re-enables without anyone pressing Stop; a node's
error ends it fail-fast with the failure naming the node instance and
what went wrong; and Stop ends it promptly, the outcome showing stopped
— including a run making no progress because a node's input never fires
(a `Format` whose `template` is neither connected nor parameterised
while its `value` is, is one way to build that). Run state — idle
or running, and the last run's outcome — is server state like the file:
a second tab and a reload show it, and a start or stop made in one tab
is visible in the others. The editor's outputs no node consumes are
discarded; attaching a console printer to what a run produces is not the
editor's business.

```sh
cargo run -p visual -- examples/visual/graphs/ticker.yml  # the long-running sample
```

The ticker sample runs a `Ticker` to a large count — 100000 values, one
every 100ms — so the run lasts while the run control is exercised: drop
and wire nodes against it and watch every editing gesture do nothing,
then press Stop and see editing return. Without it, every node the
shipped plugins build completes in microseconds and no run lasts long
enough to observe or interrupt.

The editor's graph lives in a graph file. The chrome names the file being
edited — untitled until a first save — with an unsaved-changes marker an
edit sets and open, save, and New clear; a reload or a second tab shows the
same name and marker. New returns to an empty,
untitled graph; Open loads a graph file from a path the editor asks for;
Save writes the graph to the file being edited, asking for a path only on
the first save of an untitled graph; Save as always asks. Opening a file —
another file or the current one — or starting fresh, over unsaved changes
asks before discarding them. Files are read and written server-side
through the one graph file format (`nodetool::graph`), so a file the
editor saves is a file `run-graph` runs. A load or save failure is
reported in
the status line naming the path and the fault, leaving the held graph and
file untouched. Launched on a path (`cargo run -p visual -- <file>`) the
editor opens already showing that graph; the sample ships in
`examples/visual/graphs/` and exercises the three node placements — a node
at its recorded position, a node with no position on the deterministic
fallback, and a node whose type the binary never linked rendered as an
inert placeholder.

The run-graph samples beyond the default select the demonstration:

```sh
cargo run -p run-graph failing      # fail-fast: the output stops arriving, the error names the node, exit 1
cargo run -p run-graph broken       # a load error ends the path, printed, exit 1
cargo run -p run-graph uncompilable # compile errors end the path, printed, exit 1
cargo run -p run-graph if-true      # the utility If steers to `then`, the Format template substituting
cargo run -p run-graph if-false     # the same graph steered to `else`, the Format's plain string form
```

Two things to expect from the `if` samples. The first routed value prints
more than once — the condition literal's arrival re-runs the held value and
the template literal's arrival fires its own run, so the first value prints
three times before later values print once; that is the stream semantics'
pairing, not a duplication. And a Format's `template` input must be fed for
the node to fire, `template: ""` being how a graph asks for the plain form,
while a Format on an unselected branch never fires and must carry no
template: a fed template there hangs the run instead of completing it.

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
the whole registration step; the first-party utility crate
(`crates/nodetool-utility`) is the smallest reference library written that
way. The macro's rustdoc is the guide — `cargo doc -p nodetool --open` — to
the declaration form and its linker caveat. A node type declared with the
optional `behaviour` arm runs: the named function builds a fresh behaviour per
instance from the instance's compiled shape, implementing
`nodetool::behaviour::Behaviour` against the input and output faces whose
stream semantics that module's rustdoc settles. A type declared without
`behaviour` is a descriptor alone.

A node whose ports are generic over several types declares them as one *port
family*: the ports carry the family name and the member types they span, and
the compiler resolves the family to one concrete type per instance from the
instance's connections and parameter literals — exact agreement first, then
the first declared member every source reaches. The resolved type is recorded
on the compiled node, so the behaviour builder stamps the instance for that
type; `crates/nodetool-fizzbuzz` is the reference. The optional
`check_parameters` arm names a function the compiler consults once the
instance's parameters and families resolved, for validations the type rules
cannot express — a zero step, say.

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
