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
  `Condition` operations (one node type per operation, under a condition
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
  tests' node types, `delta` supplies the engine tests' node types, `epsilon`
  supplies the custom-UI tests' declarations: a serialised custom type with
  its value UI, a node type with declared node UI, and undeclared kinds
  beside them).
- `examples/plugins` — small example plugin crates (`shapes` is sub-grouped,
  `text` is flat). The text plugin's `Ticker` is a slow source: fired by
  its `count` parameter it counts from one through the count, a fixed
  pause between values — the one shipped node a run lasts long enough to
  watch, edit against, and stop. The shapes plugin declares custom UI:
  its `shapes/shape` type carries a serialiser and a value bundle, and its
  `Shape stage` node carries a node bundle — both embedded from
  `examples/plugins/shapes/ui/` — and it ships the runnable `Shape source`
  emitter the value flow is watched at. `badui` deliberately names a
  component contract version no editor speaks.
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
- `examples/visual-badui` — the editor binary with the `badui` plugin
  linked: one deliberately unknown contract version, for a deliberate
  look at the fallback. Dropping its node renders it by the default class
  and the report names the type.

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
cargo run -p nodetool-fizzbuzz -- crates/nodetool-fizzbuzz/graphs/fizzbuzz.yml  # one hundred lines, as they arrive
cargo run -p visual            # the editor on http://127.0.0.1:8420
cargo run -p visual -- examples/visual/graphs/sample.yml  # the editor on a graph file
```

`visual` starts the editor server: open the printed address in a browser
and find the palette of every node type the linked plugins contribute on
the left and the canvas beside it — the example links the fizzbuzz
plugin crate beside the utility and example plugins, so its palette is
the headless binary's. The palette reads as an index: one section per
plugin headed by the plugin's name, a headed sub-section per declared
sub-group within it, the types without a sub-group directly under the
header — plugins, sub-groups, and types alphabetical, every tab and
reload agreeing — and each row carries the type's declared icon beside
its label. Dragging a type onto the canvas creates
a node where it dropped; pressing Enter or Space on a focused palette
row creates one at the centre of the view — the same create operation,
at a position the view names. Dragging a node moves it; a click selects,
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

Selection scales to batches. A shift-click-drag on the background draws a
rectangle and every node inside or intersecting it joins the selection; a
shift-click on a node toggles it in or out; a plain click keeps selecting
one node alone and a plain background click clears the selection — the two
background drags differing by Shift alone. With several nodes selected,
dragging any of them — a port drag staying the wire gesture above — moves
the whole selection, one move landing where each node rests, and Delete
(or Backspace) with the canvas in focus — a field keeps the keys as text,
so a stray keypress cannot cost nodes — removes every selected node
together with their wires, unknown-typed placeholders included; alone, a
placeholder stays inert. The sidebar with a multi-selection
names the selection by its count and shows only the parameter fields the
nodes hold in common — an input declared scalar-possible and unconnected
on every one of them — each at its shared value, marked `mixed` where the
values differ; a commit lands on every node, an empty commit unsets the
parameter everywhere, a field left untouched commits nothing, and where
nothing is common the sidebar says so plainly. No label editing in a
multi-selection: a label names one node.

The keyboard reaches every editing gesture the pointer does, through the
same operations. Tab walks the editor in one order — the chrome controls,
the palette rows, the canvas's nodes and their ports, the sidebar's
fields — and every focusable element shows the focus outline; a focused
node is brought into view, and so is a focused port. Enter or Space on a
focused node selects it alone, as a click does; Shift with them toggles
it in the selection, as a shift-click does; Escape is the quiet cancel —
it clears the selection, stands down an in-progress keyboard wire, and
discards an uncommitted field edit, as it already did. Arrow keys nudge
the selected nodes, Shift for the larger step, each nudge committing
the same move a drag stop commits. A focused port takes Enter or Space to
start a keyboard wire from it — from either end, a connected input
included; Tab moves the wire's candidate to the next port,
Enter or Space lands the wire where a pointer drag would — an
already-wired input replaced — and Escape stands the wire down. Delete
(or Backspace) on a focused connected input unhooks it, the drag-off's
operation. Pan and zoom stay pointer-only: focus is the keyboard's way
around the canvas. While a run is on — or the connection is gone — the
keyboard's editing paths go quiet exactly with their pointer twins, the
one lock over both input modes, while focus, selection, and navigation
stay live.

The declared data types carry the graph's type channel: a port declaring
exactly one type renders its dot in that type's declared colour and shape,
a wire renders in the colour of the source port it flows from, and a
union-declared port, an unknown type reference, or a type that does not
declare both a colour and a shape renders in one shared neutral. Colours
and shapes live with the type declarations — the base scalars' in core,
custom types' in the declaring plugin's metadata — composed into the
listing the server serves, so the browser hardcodes no type's appearance;
the declared types remain readable in a port's tooltip. Every colour the
editor renders — its own theme and the neutral pair, and the base
scalars' declarations against the surfaces they sit on — clears the
editor's contrast floor: text 4.5:1, meaningful edges 3:1, judged against
the light theme as the stylesheet declares it. A node whose type
declares custom UI renders the plugin's own body between the editor's
title bar and ports — the shell stays editor-rendered, so every gesture
on it behaves exactly as on a default node — and a value whose type
declares value UI displays through the plugin's component wherever values
display. The shapes plugin carries both, and the `Shape stage` is the
node to watch it on: wire a `Shape source` into it, give the source its
`side` value, and the staged shape shows in the stage's own body and at
the source port's readout. The stage's `note` is a second input, and it
obeys the one gate every input obeys: unwired and unfilled it never
arrives, the stage never fires, and the run holds until stopped — the
canvas marks the stage with that warning before any run — so give it its
caption through the stage's own note field; the edit lands in the sidebar
like any parameter and the run proceeds. Both bundles load only when first
used, and every failure around
them is reported naming the type, the attachment point falling back on
its own: the node to the default class, the readout to its contentless
form, the editor never blank.

Nothing is policed while editing — a graph with problems edits, saves,
and starts exactly as freely as a clean one — but the problems are shown
before any run is spent on them: after every change to the held graph,
and when a file is opened, the server recomputes the graph through the
same compile a start runs and pushes the problems it finds — errors and
warnings, each naming the nodes it speaks of — beside the definition. A
node a problem names carries a mark on the canvas, the message readable
at the mark; an unknown-typed node's placeholder carries its unknown-type
error, explaining its inertness. The one warning compile currently makes
is the hang gate: an input neither connected nor parameterised while
another of the node's inputs is — the node will never fire, and a run of
it hangs until stopped. Fix the problem and the mark is gone on the next
edit; a clean graph carries nothing. The server holds the graph: a
reload or a second tab shows the same graph and the same marks, and an
edit in one appears in the other.

The chrome carries the run beside the file controls: one control that
reads Start when the editor is idle and Stop while a run is on — the
editor session itself is the interactive mode, there is no mode to
switch on. Start hands the definition the server holds to the compiler —
every start compiles afresh, so whatever was edited last is exactly what
runs — and on a clean compile the run begins: the control flips to Stop
and every connection is pushed the new run state. A compile failure is
reported as a toast in the chrome, the errors naming what and where, the
same errors already sitting as marks on their nodes; the run never
starts, the state stays idle, and editing stays exactly as free as it
was — the editor never polices, compile decides. While a run is on the
definition is held still: palette drops, node moves, wires and
unhooking, deletion, label and parameter edits, and the file controls
are inert, and a second tab's edit is refused server-side — selection,
panning, and zoom stay live, looking not being editing. The run ends by
itself when every node completes — idle returns, the outcome shows
completed, and editing re-enables without anyone pressing Stop; a node's
error ends it fail-fast with the failure reported as a toast, naming the
node instance and what went wrong, and the failed node's mark carries
the explanation after the toast is gone, both gone when the next start
resets the canvas; and Stop ends it promptly, the outcome showing
stopped — including a run making no progress because a node's input
never fires (a `Format` whose `template` is neither connected nor
parameterised while its `value` is, is one way to build that — the
canvas warned about it before the run began). Run state — idle or
running, and the last run's outcome — is server state like the file:
a second tab and a reload show it, and a start or stop made in one tab
is visible in the others. The editor's outputs no node consumes are
discarded; attaching a console printer to what a run produces is not the
editor's business.

While the run is on, the canvas animates with it. Each node carries its
derived status in its title bar — running from its start, completed or
failed from its own ending, stopped when a failed or stopped run
abandons it — and every value an output emits travels its wires as a
pulse, a fan-out pulsing each downstream wire, and shows as small text
at the emitting output port, replaced by each new emission. Values of
the base scalars display as their plain text; a plugin's custom type
displays in the form its declared serialiser produces, through the
value bundle the type declares — and a type that declares neither
serialiser nor bundle animates the wire and carries no invented
content, exactly as it always has. Statuses and values persist after
the run ends — the failed node stays findable, the counter's last ticked value stays
evidence of what the run did — until the next start resets the canvas.
The browser coalesces the animation, keeping only the latest value per
port and letting a fast graph drop frames rather than queue a backlog,
and honours the platform's reduced-motion preference. A tab that
connects at any time — first open, reload, second tab, or a reconnect —
is given the current statuses and values beside the run state and then
joins the live stream, so every open tab animates the same run.

```sh
cargo run -p visual -- examples/visual/graphs/ticker.yml  # the streaming sample
cargo run -p visual-badui  # the editor with one deliberately unknown-contract bundle linked
```

The ticker sample is the one to watch. A `Ticker` counts to 100000 at
one value every 100ms and fans out to two condition nodes; a `Counter`
counts to a hundred and completes at once; and a `Drip` feeds a `Split`
into a `Check` whose `forbidden` parameter reads `never` by default. On
Start the nodes mark running, the pulses travel the wires, and the
value at the ticker's output ticks upward; a second tab opened mid-run
joins with the same statuses and values. Editing the guard's
`forbidden` parameter to `one` — a value the drip's second line
carries — arms it: the next run fails there, the guard marks failed,
and the drip and the splitter it abandons mark stopped; the marks
remain after idle returns and through a reload. Stop on a later run
leaves stopped marks; starting again resets the canvas and animates
afresh. The sample also keeps its editing-lock duty: while it runs,
every editing gesture is inert, until the run ends or Stop is pressed.

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
reported as a toast naming the path and the fault, leaving the held graph
and file untouched. Launched on a path (`cargo run -p visual -- <file>`) the
editor opens already showing that graph; the sample ships in
`examples/visual/graphs/` and exercises the three node placements — a node
at its recorded position, a node with no position on the deterministic
fallback, and a node whose type the binary never linked rendered as an
inert placeholder carrying the unknown-type error that explains it.

The connection is the editor's lifeline, and its loss is chrome state,
not silence: when the websocket closes — server stopped, network gone —
a banner names the loss and says the editor is trying again, over a
canvas that keeps its last-known view. The banner and the toasts are
polite live regions, so the reports the chrome carries are announced to
assistive technology as they appear, without taking focus. While
disconnected every
server-acting gesture — editing, file open and save, start and stop — is
inert, the browser holding no state that could back an undeliverable
edit, while panning, zooming, and selecting stay live. The editor keeps
trying, and when the server returns the tab rejoins by itself through
the connect-time resync — definition, file, run state, problems — the
banner clears, and a reconnect mid-run reattaches to the live state the
server holds. A first open that cannot reach the server shows the same
banner rather than the empty-canvas invitation. A greeting whose
protocol or schema versions mismatch the page's stops the trying
instead: the banner names the mismatch and reloading the page is the
advice, until that reload.

The run-graph samples beyond the default select the demonstration:

```sh
cargo run -p run-graph failing      # fail-fast: the output stops arriving, the error names the node, exit 1
cargo run -p run-graph broken       # a load error ends the path, printed, exit 1
cargo run -p run-graph uncompilable # compile errors end the path, printed, exit 1
cargo run -p run-graph if-true      # the utility If steers to `then`, the Format template substituting
cargo run -p run-graph if-false     # the same graph steered to `else`, the Format's plain string form
```

Three things to expect from the `if` samples. The first routed value prints
more than once — the condition literal's arrival re-runs the held value and
the template literal's arrival fires its own run, so the first value prints
three times before later values print once; that is the stream semantics'
pairing, not a duplication. And a Format's `template` input must be fed for
the node to fire, `template: ""` being how a graph asks for the plain form,
while a Format on an unselected branch never fires and must carry no
template: a fed template there hangs the run instead of completing it.
So each sample prints one compile warning before it runs — the
unselected Format's `template` is neither connected nor parameterised
beside its fed `value` — and the run still completes: a warning advises,
it changes no outcome.

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

A plugin can also ship UI the same way it ships nodes and types: declared on
the Rust side, embedded in the crate at build time, served by the editor
server under `/plugins/`, and loaded by the browser only when first used —
the first node of the type on the canvas, or the first displayed value. The
optional `ui` arm of `node_type!` names a `NodeUi` — the bundle that renders
that node type's body in the editor — and the optional `ui` arm of
`data_type!` names a `ValueUi`: the serialisation function called whenever a
value of the type crosses to the browser, and the bundle rendering that form
wherever the values display. The serialisation is display only — nothing
travels back. A type or node declared without `ui` changes nothing: values
cross contentless, nodes render by the default class. `examples/plugins/shapes`
declares both, and `examples/visual-badui` demonstrates the fallback.

A bundle is a plain ES module exporting its component — no build step of the
editor's involved — and is built against a versioned contract, `contract: 1`
today: the component receives the page's `h` (`React.createElement`), the
node's definition slice (label, parameters, connected inputs), the live
display state (the run status, the problem messages naming the node, the
per-port latest values with custom types already in their serialiser's
form), the `locked` flag a run holds — the bundle's own editing controls sit
inert while it is true — and two helpers, `setLabel` and `setParameter`,
which issue exactly the same edit operations the sidebar and the default
node's fields commit; there is no second editing path and no other
operation. The contract's floor for the bundle's author: render the marks
handed to it, and make its own content accessible — the editor never
sanitises or styles a plugin's UI, the way it never sanitises an icon. A
bundle naming a contract version the editor does not speak, one that fails
to load, or one whose component throws is reported naming the type, and the
attachment point falls back on its own — the node to the default class, the
value readout to its contentless form. The contract's wording lives in
`crates/nodetool/ui/src/pluginui.jsx`.

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
