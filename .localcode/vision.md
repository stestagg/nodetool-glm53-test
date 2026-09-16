# Nodetool — project vision

Nodetool is a Rust crate for showing, editing, managing, and executing
asynchronous DAG-style execution graphs, built as a small, elegant core with
everything domain-specific supplied by plugins. This vision is the backdrop
every story is written against; the requirements file says *what* in detail,
this says *why* and *where it is going*.

## The problem

A large class of programs is naturally a dataflow graph: values stream
asynchronously from producers through transformations to consumers — pipelines,
simulations, generation and processing chains, glue between services. Writing
this by hand is plumbing: wiring streams, tracking completion, propagating
errors. Existing graph tools are either heavyweight platforms with their own
runtimes, or domain-specific and closed.

Nodetool makes this class of execution a small, embeddable Rust library.
A developer defines node types in Rust; graphs are plain data files; the same
graph can run headless from a terminal or be edited and watched live in a
visual editor. The balance to hold throughout: simple, elegant, and powerful —
flexibility through extension points, not through core complexity.

## Who it is for

- **Plugin authors** (Rust developers) who define node libraries — node types,
  custom data types, icons, and optionally custom UI — without touching core.
- **Graph users** who assemble and run graphs, either headless (loading a graph
  file) or in the visual editor. The same person may be both; the tool is
  single-user and local.

The first proof of the design is a `nodetool-fizzbuzz` example: a small
first-party plugin crate and binary that builds the classic fizzbuzz as a
streaming graph, run headless or in the editor. It is the yardstick for what
"works" means, not the limit of the ambition.

## Product shape

One engine, two experiences:

- **Headless**: a binary loads a graph definition file, compiles it, and runs
  it, consuming outputs as they arrive. No UI involved.
- **Visual**: a binary serves a small HTTP server with an embedded editor.
  The UI shows a palette of node types contributed by every linked plugin,
  a canvas for arranging and connecting nodes, and a right-hand sidebar for
  editing the selected node. Interactive mode adds start/stop; while running,
  editing is disabled. The same event stream the engine emits is bridged over
  websocket so nodes animate as execution proceeds.

Core capabilities, all usable without any specific node library:

1. **Node type definition and registry** — ports (typed inputs left, outputs
   right), labels, icons, optional grouping, in the spirit of Blender's
   editor: compact nodes, minimal exposed detail.
2. **Custom data types** — a plugin can register new types, opaque to core
   beyond top-level metadata, with optional transparent conversions.
3. **Graph definitions as files** — static documents with visual metadata
   (positions etc.) attached per node, editable by hand or by the editor.
4. **Compilation** — a graph definition becomes a compiled, immutable,
   executable graph; type resolution happens here, before execution.
5. **Streaming execution engine** — fully asynchronous, values propagate as
   they become available, with an optional events observer.
6. **Server + editor UI** — a react-flow style editor over websocket, light
   blueprint-themed look.
7. **A first-party library of trivial utility nodes** (home: see open
   questions) — e.g. an `if` router, a `Format`-to-string node. Core itself
   carries no real node implementations beyond this kind of trivial utility.

A user's editing loop: drag a node from the palette onto the canvas, connect
output to input (fan-out allowed; one upstream per input), edit parameters in
the sidebar — scalar inputs can be typed inline until a connection replaces
them — then press start and watch values flow. Editing includes undoing
structure as well as building it: removing an edge (unhooking the input back
to its scalar field) and deleting nodes; the exact gestures are story detail.
Fixing a mistake means stop, edit, recompile (fast), restart.

## Scope

**In**: the core crate (types, registry, graph model, compiler, engine,
events, server, UI), plugin registration via `inventory`, the core scalar type
set and trivial conversions, the trivial utility node library, and the
fizzbuzz example as a first-party plugin crate.

**Deliberately out**:

- Distributed or multi-machine execution; one process runs one graph.
- Multi-user, authentication, remote access; the server is a local tool.
- Persistence: run history, databases, dashboards. A run's value is consumed
  as it happens.
- Live modification of a running graph; the answer is recompile-and-restart.
- Cyclic graphs; the model is a DAG.
- Automatic coercion to string; string formatting is an explicit node.
- Node authoring from non-Rust languages; plugins are Rust crates.
- A marketplace or dynamic plugin loading; plugins link in at build time.
- Long-term graph file migration tooling; a schema version field is enough
  for now.

## Architecture and technology direction

**Workspace.** A cargo workspace: the `nodetool` core library crate, plus
separate crates for first-party plugins and examples such as
`nodetool-fizzbuzz` (a binary crate that depends on core, registers its
nodes, and offers headless and UI modes). The boundary is strict: core
provides the abstract machinery — node model, type model, graph model,
compiler, engine, events, server, editor — and nothing that names a concrete
plugin node or type.

**Plugin registration via `inventory`.** Node libraries are ordinary crates;
linking them into a binary is the only integration step. `inventory` lets
them contribute node type descriptors, custom type descriptors, and (later)
UI customisations at static-initialisation time, with no manual registry to
maintain. The trade-off — registration only from statically linked code — is
accepted: the product's shape (build a binary that bundles your nodes) fits
it.

**Type system.** Types are identified by id/uuid and name; core defines a base
set of scalars aligned with Rust's (`i32`, `f64`, `bool`, `String`, ...). A
type may carry an optional conversion function/matrix describing cheap
adaptations to other types, so trivial conversions (`i16`→`i32`, `f32`→`f64`,
`i32`→`f64`) just work, without format-changing surprises beyond what the
conversion declares: compile time decides — resolving each connection's type
and inserting the conversion adapter — and values are converted as they flow
at runtime, literals being the trivial exception. Custom types are opaque to
core, which sees only top-level metadata (name, id); serialisation and any
rendering are entirely plugin concerns, reachable by the plugin inside its
own node UI.

**Graph definitions.** A graph is a static file (YAML as the working choice):
nodes carrying instance uuid, type reference, parameter values, and metadata
(layout position etc.), plus edges from outputs to inputs. The file format is
versioned. It must be hand-writable — headless users should not need the UI.

**Compilation.** A pure, fast transformation from definition to an executable
plan: validate structure (DAG, ports exist, types resolvable), resolve the
type of each connection (ports may accept unions of types), and wire the
stream plumbing. The compiled graph is immutable while running; the engine
routes by node instance id — streams are wired and events reported per node —
but never branches on what a node is called or which type it is. Editors may
warn early, but enforcement happens at compile time — late enough to never
block free editing, early enough that a run never starts on a broken graph.

**Engine.** Built on tokio. Connectors are asynchronous streams: the model
distinguishes "a new value arrived" from "here's the current value", and
"complete" from "more data may still arrive". A constant is just a stream that
yields once and completes. Nodes process values as they arrive, propagate
results downstream, and complete when their inputs are complete and
exhausted; a run finishes when every node is complete. Outputs fan out to any
number of downstream inputs.

**Events.** The engine takes an optional events observer (generic/dyn trait)
with callbacks for lifecycle and per-node happenings — started, value flow,
status, errors, completion. It is a plain observer: the engine neither knows
nor cares who listens. The websocket bridge for the UI is simply one
subscriber; headless runs may pass none, or a subscriber that prints.

**Server and UI.** A small HTTP server (loopback by default) serves the
static UI and speaks websocket. The UI is React with react-flow for the
canvas and the Blueprint component library for the light, blueprint-themed
chrome. The default node rendering is generated from the type definition —
inputs left, outputs right, controls in the middle, scalar helper inputs
editable inline — so plugins get a usable node for free. On-node scalar
fields and the sidebar's parameter fields are two views of one stored value:
either can be edited, and a connection replaces the editable field with a
connected/type indicator in both. A plugin may supply custom UI for a node
type, and then it also owns that UI's data handling.
The UI must remain usable when a plugin provides nothing but the definition,
and it has no silent states: a graph file that fails to load or parse is
reported visibly in the editor, first open shows a deliberate empty canvas,
and losing the server connection mid-run is surfaced clearly (whether the UI
can reattach to a running graph is a story-level call).

**Groups and subgraphs.** Designed in from the start, even if implemented
later: a group is a node whose behaviour is a nested graph, packaged with
exposed ports. The node model (a node type backed by a subgraph), the file
format (a group's definition must be expressible), the compiler, and the UI
(a group collapses to a single node with ports) must all leave room for this,
so it never forces a rework.

## Principles and constraints

- **Core stays abstract.** Core never switches behaviour on a specific node
  type, name, or id. Every change asks: is this a general capability, or a
  plugin concern? Plugin-specific concerns live in plugins, including the
  utility nodes that only make sense for particular data shapes.
- **One of each mechanism.** One node model, one type model, one event model.
  Avoid parallel paths (e.g. separate "constant" plumbing distinct from
  streams, or a second registry beside `inventory`).
- **Errors are raised, not swallowed.** Report and propagate; a silent `None`
  is a bug. Validation exists only where correctness requires it — compile
  time for graphs — and the editor's job is to warn early, not police.
- **Streaming is the truth, not an optimisation.** Semantics are defined in
  terms of value arrival and completion; anything that looks like a global
  barrier or batch pass is a design smell.
- **Small core, small code.** Prefer the smallest abstraction that accommodates
  the likely future (plugins, custom types, subgraphs); no speculative
  generality. Recompiles must be fast and efficient; the edit–stop–recompile–
  restart loop is a first-class path, not an afterthought.
- **Security posture.** Local, single-user, no auth. The server binds loopback
  by default; graph files and websocket messages are untrusted input at the
  parse boundary — malformed input is reported, never crashes the process.
- **Quality.** Idiomatic, clippy-clean Rust; engine semantics (value arrival,
  completion, fan-out, type resolution) carry focused tests; UI kept compact
  and information-dense per the Blender reference, with a basic accessibility
  floor: keyboard operability, visible focus, sufficient contrast.

## Open questions

Later stories settle these; the vision only insists they are settled
deliberately, in the open:

- The exact stream API surface a node author programs against: how "new value"
  vs "current value" and completion are expressed, the backpressure policy
  between nodes, and — for a node with several inputs — what one arrival
  means: act on each input independently, zip across inputs, or pair against
  the held current value.
- Mid-run error behaviour: fail the whole run fast, or isolate to the failing
  node and let the rest drain — and what each looks like in the UI.
- Union-typed ports: how a connection's concrete type is negotiated at compile
  time, how declared trivial conversions (`i32`→`f64`) participate in that
  negotiation, whether an applied conversion exists explicitly in the graph
  model and saved file or stays implicit, and how conflicts surface in the
  editor.
- The precise YAML schema (and what lives in per-node metadata vs parameters).
- The websocket protocol shape: the message set for editing operations and
  event streaming, and how plugin-supplied custom UI assets reach the browser.
- How node definitions express generic numeric ports (e.g. a Counter that
  supports all numeric types) without per-type duplication.
- Groups: subgraph representation in files, compile-time inlining vs nested
  execution in the engine, and what a running group exposes to the UI.
- Whether the trivial utility nodes ship inside core or as a separate
  first-party library crate, and which ones.
- Multi-selection editing: how the sidebar behaves when several nodes are
  selected (common fields only? which?).
- How much runtime data the event stream carries to the UI (every value,
  samples, or metadata only) to keep visualisation cheap.
