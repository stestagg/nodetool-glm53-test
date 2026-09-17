---
title: Sidebar node editing
date: 2026-09-16
pr_id: 49
---

## Description

Stories 12 and 13 gave the user structure: nodes dropped, arranged, wired,
unhooked, deleted — but nothing inside them. Every node wears its type's
default label, and no input can hold a value, so the graph is an arrangement
of types, not yet the user's own. This story puts the data in: the
right-hand editing sidebar story 12 kept the right edge free for, and the
inline scalar fields the node rendering has so far left blank. After it,
the user can name their nodes and type values into them — the last piece
before the editor holds a graph worth running.

The sidebar is REQ-56's shape: a right-hand dock that appears when a node is
selected and closes when the selection clears, so an unselected canvas keeps
its full width. Global node attributes sit at the top — the node's label,
editable to any name, with the node's type reference and uuid shown read-only
beneath it for orientation — and the per-input parameter fields sit below.

The label is REQ-40's half story 12 left out: the user can override the
type's default to any name, and clearing the override returns the node to its
default label — an empty field means "default", no separate reset control.

The parameter fields follow one general rule from the type listing (11): an
input whose declared types include a core base scalar gets an editable field,
whatever union it declares; an input declared only on plugin custom types gets
none, because that data is opaque to the editor (REQ-33) — wiring is how such
an input receives values, and every input accepts a connection (REQ-57). The
listing as 11 settled it carries each port's declared type references with
nothing marking which are core base scalars — the base set is declared through
the same mechanism a plugin's types are — so the classification the rule needs
rides the listing too: one flat fact per type reference, base scalar or not,
computed server-side where core's own scalar set is known (REQ-29), an
additive change to the listing message 11's framing exists to allow, while
the rule itself stays in the UI, a predicate over the listing that names no
node or plugin (REQ-73).
Outputs get no fields: they are sources, not value holders. A committed field
becomes the input's parameter value in the definition — the same plain scalar
the file format (03) carries — applied server-side and pushed to every
connection.

The on-node inline fields and the sidebar are two views of one stored value,
as the vision says: each scalar-possible input also shows a small editable
field on the node itself, and an edit in either view appears in the other.
When a wire lands on an input, the editable field is replaced by a
connected/type indicator in both views — there is nothing to type into while
the value comes from the wire, so the definition never holds an input with
both a wire and a literal (REQ-58). When the wire is unhooked (13's
gesture), the editable field returns — empty: a replaced literal is not
remembered, what the definition holds is the only state.

Nothing is policed at edit time: a typed value is read as a plain scalar
exactly as the story 03 loader reads a hand-written one — boolean, integer,
float, else string — and stored as that kind, whether or not it suits the
port; compile time judges (04) and the editor warns early with story 18. The
mirror direction holds too: an existing boolean, integer, or float parameter
renders as its scalar's text in both views, so the two views of one stored
value hold for every kind, not strings alone. The gestures from 13
keep working around this story's fields; what is still missing is everything
after: files (15), start/stop and edit-locking while running (16), live
events (17), early warnings (18), palette presentation (19), multi-selection
(20), custom UI (21).

## Definition of done

- Selecting a node opens the right-hand sidebar with the node's global
  attributes on top: the label editable to any name — the node's title shows
  the override everywhere immediately — and clearing the override returns the
  node to its type's default label; the node's type reference and uuid are
  shown read-only for orientation. Clearing the selection closes the sidebar;
  selecting a different node swaps its contents. (REQ-56, REQ-40, REQ-41)
- Below the attributes, each input whose declared types include a core base
  scalar gets a parameter field, whatever union it declares; inputs declared
  only on plugin custom types are absent from the sidebar, their data opaque
  to the editor, and outputs get no fields. Committing an edit (Enter or
  leaving the field; Esc discards an uncommitted one) stores the value as
  that input's parameter in the definition; committing an empty field removes
  the parameter — an unset input is unset, the same empty-means-unset rule
  the label override follows — applied server-side and pushed to every
  connection. (REQ-56, REQ-57, REQ-58, REQ-33)
- Each scalar-possible input also shows a small editable field inline on the
  node itself, per the default node class; clicking it edits directly, with
  no extra mode. The node field and the sidebar field are two views of one
  stored value: an edit in either appears in the other and in every open tab,
  and an existing boolean, integer, or float parameter renders as its
  scalar's text in both views. (REQ-38, REQ-47)
- When an input is connected, its editable field is replaced by a
  connected/type indicator in both views; while connected there is no field
  to type into, so the definition never holds an input with both a wire
  and a literal, and a parameter edit arriving for a connected input is
  answered with an error naming the input, the definition untouched.
  Unhooking returns the editable field, empty — a replaced literal is not
  restored. (REQ-58, REQ-21)
- No edit-time validation of values: a committed value is stored as the plain
  scalar the file format reads the typed text as — boolean, integer, float,
  otherwise string — whether or not it suits the port, and a committed value
  that is not a plain scalar of the kinds the file format allows is answered
  with an error, the definition untouched — a protocol-side guard in story
  11's framing, since no text field can commit one, and an empty field never
  reaches it: committing one removes the parameter, per the commit rule
  above. Compile time judges the graph; early warnings are story 18's.
  (REQ-60, REQ-72, REQ-71)
- A node whose type reference is missing from the listing shows the sidebar
  with label editing only — no invented parameter fields. (REQ-71)
- The browser holds no graph state of its own: reloading, or a second tab,
  shows the same labels and parameter values, and an edit made in one tab
  appears in the other. (REQ-2, REQ-45)
- Focused tests cover the new operation messages server-side: a label edit
  stores the override and clearing it removes it; a parameter edit stores the
  scalar on the named input, the stored kind the file format's — a committed
  `2.5` a float, a committed `true` a boolean, on a port of any declared
  union — and committing an empty field removes the parameter; an edit for a
  connected input, and a value that is not a plain scalar, are answered with
  errors leaving the definition untouched and the connection usable, and so
  is an edit naming a node the held definition does not contain, per story
  11's framing, while an input name the node's type does not declare is
  stored, not refused — the file format carries it and compile time judges,
  no error path wanted there; the listing carries the base-scalar
  classification the field rule reads; pushes reach every connection.
  The workspace builds and passes `cargo test` and `cargo clippy` cleanly.
- The visible proof, run per DEVELOPMENT.md: open the editor, drop a node,
  rename it in the sidebar and see the title change, type a value into a
  sidebar field and see it appear on the node, type into the node's inline
  field and see it in the sidebar, wire over the input and watch both fields
  become indicators, unhook and see the empty field return, clear the label
  override and see the default return, then reload and check a second tab
  shows the same.

## Comments

- 2026-09-16 — Scope seams: this story's additions to story 11's message
  catalogue are label and parameter edits, plus one flat fact on the listing —
  per type reference, whether it is a core base scalar — computed server-side,
  an additive change 11's framing allows (REQ-29); wire/unhook/delete are
  13's, files 15's, start/stop and the lock that disables these fields while
  running 16's (REQ-25), live events 17's, early error surfaces 18's,
  multi-selection's common-fields sidebar 20's, plugin custom UI 21's.
  REQ-40 completes here —
  story 12 rendered the default label and named the override as this story's.
  (REQ-74)
- 2026-09-16 — UX calls settled here: an empty label means "default", so
  reverting needs no reset control, and the same empty-means-unset rule
  governs the parameter fields — committing an empty field removes the
  parameter, the input left unset, the definition holding no empty-string
  stand-in — both inherited by 20's common-fields sidebar rather than
  relitigated there; non-scalar inputs are absent from the sidebar rather
  than shown as dead widgets — the canvas ports show them, and the sidebar
  carries only what can be edited (REQ-47); unhooking does not
  restore a replaced literal, because 13 kept no memory of it and adding one
  now would make the definition not the only truth; fields commit on
  Enter/blur and Esc reverts, so typing is not broadcast per keystroke; the
  uuid is shown read-only, not offered as an editable attribute — it is the
  node's internal identity (REQ-41), useful for correlating with files and
  error messages.
- 2026-09-16 — Why both inline fields and a sidebar: the vision calls them
  two views of one stored value and neither substitutes for the other —
  inline fields keep values one click away on compact nodes (REQ-47), the
  sidebar gives the full attribute view one place. Splitting them across
  stories would land half a feature either way, so both arrive here.
- 2026-09-16 — Why nothing is validated at edit time: same line as story 13 —
  the editor must never block free editing (REQ-60), a value that does not
  suit its port is the user's to fix via compile feedback (18's warnings), and
  rejecting edits would be an extra failure mode for no correctness gain
  (REQ-72). The only refusals are structural: a connected input cannot also
  hold a literal, and the definition cannot hold a non-scalar parameter,
  since both would contradict the file format (03) the definition mirrors.
- 2026-09-16 — Which inputs get fields is decided from the type listing
  alone — declared types including a core base scalar — so core never
  branches on a specific node or plugin (REQ-73), and a plugin gains the
  sidebar for its nodes for free by declaring scalar ports, the same way it
  gains the default node rendering (REQ-38). The listing itself carries the
  base-scalar classification as one flat fact per type reference, computed
  server-side where core's own scalar set is known (REQ-29); the alternative
  of the browser hardcoding the scalar names would be a second registry,
  silently rotting the day the base set grows. Custom-type rendering in the
  sidebar is plugin territory via story 21.

- 2026-09-17: Implemented in pull request #49 (http://localhost:8080/gitea/localcode/nodetool/pulls/49).
