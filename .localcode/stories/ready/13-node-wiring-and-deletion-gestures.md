---
title: Node wiring and deletion gestures
date: 2026-09-16
---

## Description

Story 12 gave the user a stage and the first gestures: nodes can be dropped
onto the canvas and arranged, but nothing connects to anything, and a
misplaced node cannot even be removed. This story completes the
build-and-unbuild loop on the canvas: wiring outputs to inputs, unhooking a
wire, and deleting nodes — the structural editing gestures, applied
server-side exactly like create and move. After it, the user can build and
revise real graph structure; what is still missing is the data inside the
nodes (inline scalar fields and the sidebar, story 14), so an unhooked input
returns to its plain unconnected port for now.

The gestures, drawn as directly as possible from how the Blender reference
behaves:

- **Wire**: dragging between an output port and an input port — from either
  end — creates an edge, drawn live during the drag and landed as an edge in
  the definition when released on a port. Releasing anywhere the wire cannot
  land — anywhere that is not a port of the other kind — cancels quietly: no
  edge, no error noise. A drag that grabbed a connected input's end is the
  unhook gesture below, not a cancel: releasing it off-port removes the old
  wire rather than restoring it. One output can feed any
  number of inputs, drawn as separate wires; an input takes at most one
  upstream, so dropping a new wire onto an already-wired input *replaces* the
  old wire rather than demanding an unhook first — the most natural gesture
  should not be a run-in with an error dialog.
- **Unhook**: dragging a connected input's end off and releasing it on
  anything that is not a port removes that wire — the input returns to
  unconnected. Released on a port, the same drag is the wire gesture, and
  replace semantics do the re-routing. Unhooking an input that has no wire
  changes nothing; there is no separate edge-selection-and-delete path.
- **Delete**: pressing Delete (or Backspace) while a node is selected removes
  it from canvas and definition together with every wire attached to it —
  no dangling edges are left behind, since the file format (03) forbids them.

The server stays the sole owner of the graph, as in story 12: each gesture is
an operation message — this story's additions to story 11's catalogue —
applied to the definition the server holds and pushed to every connection. A
wire landing on an input that carries a parameter literal replaces the
literal, because an input has one value source, connection or literal, never
both; unhooking simply leaves the input unset (whether a replaced literal can
be remembered and restored is story 14's call, when fields exist to type into).

Nothing checks types, port validity, or cycles at edit time: every wire is
drawn freely, even one that runs a node back into itself. Compile time judges
(04) when a run is started (16); early warnings are story 18's. The editor's
job is to never block free editing, not to police.

Deliberately not here: inline scalar fields, the sidebar, and label editing
(14); file load/save (15); start/stop and the edit-locking that disables
these gestures while running (16); early error surfaces (18); palette
presentation (19); multi-selection and group delete (20).

## Definition of done

- Dragging between an output port and an input port (from either end) creates
  an edge rendered on the canvas between the two ports; the gesture becomes a
  wire operation naming the edge it means — source node by uuid and output
  port by name, target node by uuid and input port by name — applied to the
  server-held definition and pushed to every connection. One output feeds
  any number of downstream inputs, each shown as its own wire. (REQ-54,
  REQ-20, REQ-41)
- An input has at most one upstream: dropping a wire on an already-wired
  input replaces the old wire everywhere it is visible, and a wire landing on
  an input that holds a parameter value replaces the literal, so the
  definition never holds an input with both. A connected input shows a
  visible connected state on the node. (REQ-21, REQ-58)
- Dragging a connected input's end off and releasing it anywhere that is
  not a port unhooks it: the edge is removed from definition and canvas in
  every open tab, and the input returns to its unconnected state (its
  editable scalar field returns with story 14); unhooking an input with no
  wire changes nothing. Every input of a known type can be wired — nothing
  is forbidden to connect — and no type, port, or cycle checking happens at
  edit time. (REQ-57, REQ-60, REQ-72)
- Pressing Delete (or Backspace) on a selected node removes it from the
  canvas and the definition together with every edge attached to it, leaving
  no dangling edges. (REQ-2)
- The browser holds no graph state of its own: reloading, or a second tab,
  shows the same wiring, and a wire, unhook, or delete made in one tab
  appears in the other. (REQ-2, REQ-45)
- Focused tests cover the new operation messages server-side: wire creates
  the edge and replaces an occupied input's edge and any literal it held,
  and a wire from a node's output to one of its own inputs lands at edit
  time, compile (04) being what rejects it later; unhook removes the edge
  and is idempotent; delete removes the node and
  cascades its edges; operations naming a node not in the definition are
  answered with an error per story 11's framing, leaving the connection
  usable; pushes reach every connection. The workspace builds and passes
  `cargo test` and `cargo clippy` cleanly.
- The visible proof, run per DEVELOPMENT.md: open the editor, drop a few
  nodes, wire a fan-out from one output to two inputs, release a drag over
  empty canvas and see it cancel quietly — no wire, no error — drop a new
  wire onto an occupied input and watch the old wire replaced, drag a wire's
  input end off to unhook it, wire an output back into its own node's input
  and watch it land, delete a wired node and see its wires go with it, then
  reload and check a second tab shows the same graph.

## Comments

- 2026-09-16 — Scope seams: this story's additions to story 11's message
  catalogue are wire, unhook, and delete (rewiring is wire with replace
  semantics — no second message); parameter edits are 14's, refining 11's
  comment that split them across 13 and 14. The vision's phrase "unhooking
  the input back to its scalar field" completes in 14, when fields exist:
  what 13 guarantees is that unhooking leaves the input unconnected and
  unset. Run-locking these gestures is 16 (REQ-25); early compile-error
  surfaces are 18; multi-select delete is 20; the a11y pass over the
  keyboard gestures is 22. (REQ-74)
- 2026-09-16 — UX calls settled here: a wire dropped on an occupied input
  replaces it, because blocking the natural gesture to force an unhook first
  is ceremony; both drag directions start a wire since the user's hand is
  often nearer the input; drag-off is the one edge-removal mechanism (the
  Blender detach gesture), so edge selection and a second delete path are
  not introduced; Delete/Backspace on the selection is the deletion gesture,
  already keyboard-operable. There is no undo mechanism anywhere in the
  plan — the vision's fix path is stop, edit, recompile, restart, and
  unbuilding structure (unhook, delete) *is* undoing.
- 2026-09-16 — Why nothing is validated at edit time: the vision is explicit
  that enforcement happens at compile time, late enough to never block free
  editing (REQ-60), and the editor warning early is story 18's work; a
  questionable wire that lands freely is exactly the intended behaviour, and
  adding wire-time rejection would be an extra failure mode for no
  correctness gain here (REQ-72). Cycles, unknown ports, and type
  mismatches all wait for compile.
- 2026-09-16 — A wire replacing an input's parameter literal mirrors story
  03's rule that an input carries a connection or a literal, never both; the
  replaced literal is not resurrected on unhook — whether replaced values are
  remembered at all is story 14's call, and 13 keeps no second bookkeeping.
  Unknown-typed placeholder nodes from story 12 offer no grabbable ports, so
  no wire gesture can start or land on them — a rendering fact, not a check:
  the server applies an operation naming a placeholder like any other, and
  compile time judges (04). They take no part in wiring because the wire
  operation names ports by name and a placeholder's ports are unknown — no
  name to name, no carved-out exception. All operation names travel as uuid
  and port names — nothing here knows what a node is, only what the
  definition holds. (REQ-73)
