---
title: Groups and subgraphs
date: 2026-09-16
pr_id: 60
---

## Description

Stories 03 and 04 kept the promise the vision made in the same breath as its
scope: groups and subgraphs are designed in from the start, even if
implemented later — the file format stayed uniform so "a group will enter as
an ordinary node instance whose type reference names a group type", and the
compiler's shape left room for validation and type resolution recursing into
a nested body. What nothing yet does is make a group *exist*. Today a user
whose graph carries a section that is one thing in their head — a decision,
a filtering stage, a whole side-branch — can only watch the canvas fill with
nodes and wires that never collapse into the one thing they mean. This story
makes groups real: a graph file can define a group — a named, reusable node
type whose behaviour is a nested graph, packaged with exposed ports — and
instances of it sit on the canvas and run like any node, collapsed to a
single node with ports, while the same file runs headless unchanged. After
it, complex logic can be packaged up (REQ-44) for the first time.

The three questions the vision left open, settled here in the open:

- **Subgraph representation in files: named groups in the same document.**
  The document gains an optional `groups` section; each group carries a name
  unique in the document, its exposed ports — inputs and outputs, each a
  name, declared type references (unions allowed, the same port vocabulary
  as any node type), and the inner node and port it binds, the edge it stood
  in for at packaging time — and a nested graph: nodes and edges in exactly
  the top-level shapes. A group instance is an ordinary node instance, the
  entry story 03 reserved: type reference naming a group of the same
  document, label, parameters, and metadata as any node's. Groups may
  instantiate other groups; a group that, directly or through others,
  instantiates itself is an error like a graph cycle. A group's graph is
  closed: its inner edges land only on its own nodes, so everything crossing
  the boundary crosses as an exposed port. The reader speaks schema versions
  1 and 2 — a version 1 file means exactly what it always meant, and a
  version 1 document carrying a `groups` section is a load error naming the
  version — and writes version 2; round-tripping carries groups verbatim,
  inner metadata included.
- **Compile-time inlining vs nested execution: flatten at compile.**
  Compiling a definition with group instances produces one flat compiled
  graph — the only shape the engine has ever run. Inner nodes take
  identities derived deterministically from the chain of group instances
  enclosing them plus the node's own uuid, so an instance uuid legally
  reused under different enclosing chains derives distinct identities;
  boundary edges are rewired so an exposed input receives the group instance's
  external upstream — or its parameter literal, a literal on an exposed
  input flowing inside exactly as any node's parameter does — and an exposed
  output feeds the external downstreams; each binding is an ordinary
  connection under the ordinary rules, the exposed port's declared types
  resolving against the inner port's exactly as any edge does, declared
  conversions included. Rationale: nested execution would give the engine a
  second execution mode and a second event-routing story beside the one it
  has — the parallel path the vision forbids — while flattening is pure,
  in-memory recursion that keeps recompiles fast (REQ-26). Story 04's
  accommodation was room, not a nested structure; flattening uses the room
  and reworks nothing.
- **What a running group exposes to the UI: the boundary, and an honest
  aggregate.** The group's ports and wires animate like any node's — they
  are real connections in the flat run. The collapsed node's status mark
  aggregates its inside: error when an inner node errors — the run's
  fail-fast ends the run, and the report names the group instance, label
  and derived identity per 07's uuid-where-labels-collide pattern, and the
  inner node, so an inside mistake is findable from the outside; completed
  when every inner node is completed; stopped when a stopped or failed run
  closes started inner nodes without their own final transition — 17's
  closure rule applied to the inside; running only while inner nodes are
  running. Inner value pulses stay inside — collapsed means collapsed, and
  unpacking is the way in (25).

The editor consumes groups without a new mechanism. A group instance renders
by the same default node class as any type, its type reference resolved
against the document's groups before the node-type listing's — a group
instance never renders as 12's unknown-type placeholder: label defaulting
to the group's name and overridable, ports from the group's exposed ports
with 19's type channel, inline scalar fields on scalar-possible unconnected
exposed inputs, connected indicators when wired. The sidebar edits the
instance like any node (14), and the gestures already shipped work on it
untouched: wiring
(13), moving, selecting, deleting the instance — whose cascade removes the
instance, never the group's definition. The palette stays a palette of
linked plugin types: groups arrive with the document, not the registry, so
they take no palette entry. The protocol needs nothing: the definition
message carries the groups section verbatim (11's settlement) and the
browser composes the collapsed node from what it already holds — no new
message, no second path. Headless needs nothing either: the same file loads,
compiles, and runs (06, 10), outputs printed as they arrive — the
flattening makes the two experiences identical by construction.

Deliberately not here: creating and unpacking groups on the canvas — the
packaging gestures that make groups a flow rather than a format — are story
25; entering a group to view or edit its inside on the canvas (unpacking is
the way in; an enter-group view is a future story if the packaged-up loop
asks for it); editing a group's exposed ports after creation
(unpack-and-repack covers it); groups in the palette; any fizzbuzz sample
reshaped around groups (24's call).

## Definition of done

- A graph file can define groups: an optional `groups` section whose entries
  each carry a name unique in the document, exposed input and output ports —
  a name, declared type references, and the inner node and port each binds —
  and a nested graph of nodes and edges in the same shapes as the top level;
  a group instance is an ordinary node instance whose type reference names a
  group in the same document, carrying label, parameters, and metadata like
  any node; groups may instantiate groups, and a document's groups form no
  cycle. The reader still loads version 1 files exactly as before — a
  version 1 document carrying a `groups` section is a load error naming the
  version — writes version 2, and a load–dump round trip preserves groups
  verbatim — names, ports, bindings, inner nodes, inner edges, inner
  metadata. (REQ-44, REQ-11)
- The new shape fails loudly and precisely, each error naming the group and
  where it sits — at load, where the check is document-local: a duplicate
  group name, an inner edge landing on a node outside its own group, a
  binding naming an inner node its group does not contain; at compile, where
  the registry or the resolved types are needed: an instance referencing a
  group the document does not define, a group name colliding with a linked
  plugin's type reference, a binding naming an inner port its node does not
  have, a group-reference cycle, an exposed port whose declared types cannot
  bridge to the inner port it binds. Uuids must be unique within each graph;
  collisions across groups are legal, namespaced at compile. A group no
  instance references is not an error — dead weight is the hand-writer's to
  see, not a failure mode to invent; a group-reference cycle is the
  exception, an error wherever it sits, since nothing instantiating a member
  of one can ever compile. (REQ-71, REQ-72)
- Compiling a definition with group instances yields one flat compiled graph
  that behaves identically to the same graph written flat by hand: inner
  nodes with identities derived deterministically from the chain of group
  instances enclosing them plus the node's own uuid, boundary edges rewired
  to the external world, an unconnected exposed input
  carrying a parameter literal fed by it through the binding, types resolved
  across the boundary under the same rules as any connection with declared
  conversions applied. Compilation stays pure and fast — in-memory
  recursion, no I/O — and the engine runs the flat graph with no knowledge
  that groups exist. (REQ-44, REQ-15, REQ-2, REQ-26, REQ-73)
- Events for nodes inside a group carry their compiled identities — derived
  deterministically from the chain of group instances enclosing them plus
  the node's own uuid, so which group instance and inner node an identity
  names is recoverable without the engine knowing groups exist — and the
  terminal subscriber's event timeline and the run's error report label them
  accordingly, naming the group instance by label and derived identity, a
  grouped run as inspectable as a flat one. (REQ-13)
- In the editor, a file with groups opens showing each group instance as one
  collapsed node carrying the group's ports — rendered by the same default
  node class as any type, with 19's type channel on its ports, inline scalar
  fields on scalar-possible unconnected exposed inputs, connected indicators
  when wired — and the shipped gestures treat it like any node: wire, move,
  select, label and parameter edits in the sidebar, delete removing the
  instance only. An instance's type reference resolves against the
  document's groups before the node-type listing's — a group instance never
  renders as 12's unknown-type placeholder. The palette lists linked plugin
  types only. No protocol message changes: the definition the browser holds
  carries the groups verbatim. (REQ-44, REQ-37, REQ-38, REQ-56, REQ-58,
  REQ-47)
- Running a grouped graph animates the group's boundary ports and wires like
  any node's, and the collapsed node's status mark aggregates its inside —
  error when an inner node errors, the report naming the group instance and
  the inner node; completed when every inner node is completed; stopped when
  a stopped or failed run closes started inner nodes without their own final
  transition, 17's closure rule applied to the inside; running only while
  inner nodes are running, a never-run group showing no mark, the bridge
  holding no statuses before the session's first run — while edits to a
  group instance go quiet under the run lock with every other edit.
  (REQ-14, REQ-25, REQ-44)
- The same grouped file runs headless: loaded, compiled, executed with
  outputs printed as they arrive. (REQ-7, REQ-44)
- Focused tests cover: the round trip with groups; each load and compile
  error path above, respectively; flattening — identities, boundary rewiring,
  a parameter flowing
  through the boundary, type resolution and conversion across it, the
  group-cycle and collision errors, and flat-equivalence against the same
  graph written without a group; event namespacing. The workspace builds and
  passes `cargo test` and `cargo clippy` cleanly, with the UI build part of
  the check set DEVELOPMENT.md documents; the status aggregation rides the
  UI test runner 20 added, alongside 17's marks.
- The visible proof, run per DEVELOPMENT.md: the editor example's sample
  files gain one with a group — open it and see the collapsed node among
  ordinary ones, identifiable as a group instance by the group's name as its
  default label, no icon in its title bar where every ordinary node shows
  its type's, and 14's read-only type reference in the sidebar naming the
  group; ports and wires as normal; wire a node to one of its exposed
  inputs, run, and see the boundary animate; break something inside a group
  in the file, reopen, run, and see the run stop with the error naming the
  group instance and the inner node; run the same file headless from a
  terminal and see the same outputs printed as they arrive; save and reload
  from the editor (15) and see the groups come back verbatim.

## Comments

- 2026-09-16 — Scope seams: the packaging gestures that create and unpack
  groups on the canvas are story 25, built on 20's selection; this story
  adds no gesture, so 22's keyboard pattern applies to 25's when it lands,
  not here. Story 19 anticipated "a group node's palette entry is an
  ordinary declared type" — settled the other way: groups are
  document-local, not registry types, so the palette promise stays
  plugin-types-only and a group's palette-shaped entry never exists. The
  protocol takes no message: 11's note that whatever schema room groups
  need rides the definition message untouched is kept literally. (REQ-74)
- 2026-09-16 — Why named groups in the document rather than a nested
  document under each instance: a packaged thing you cannot reuse is barely
  packaged — naming lets one group serve many instances; the name gives its
  instances a default label; and the interface is local, ports declared on
  the group and checked by compile against the inner ports they bind,
  rather than derived by walking the nested graph. It is also exactly the
  entry story 03 reserved: an ordinary node instance whose type reference
  names a group type.
- 2026-09-16 — Why flatten: the vision's one-of-each-mechanism line. Nested
  execution means a second execution mode in the engine and a second story
  for events and statuses; flattening means the engine — and the run
  semantics 05 and 06 settled — apply unchanged, and the new reasoning lives
  only in the compiler, where purity and determinism are already the
  contract. The cost is honest: inner identities are synthesised at compile,
  so events and errors reach the user labelled with the group instance and
  inner node they name — the naming recovered from the compiled identity by
  whoever prints, never a new engine payload — rather than raw definition
  uuids.
- 2026-09-16 — Version handling: the reader's supported versions become
  {1, 2}, saves write 2, and a version 1 document means exactly what it
  always meant — the version gates the shape, so a version 1 document
  carrying a `groups` section is a load error naming the version, never a
  v1 file silently gaining meaning. This is the first use of the version
  field for growth, and it is additive only — the vision's migration stance
  (a version field is enough, no tooling) respected.
