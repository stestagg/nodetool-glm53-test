---
title: Packaging groups in the editor
date: 2026-09-16
---

## Description

Story 23 made a group exist: a graph file can define named groups with exposed
ports, their instances collapse to one node and run — headless and in the
editor — and the editor consumes them with no new mechanism. What nothing yet
lets the user *do* is make one. Today a user whose canvas carries a section
that is one thing in their head — a decision, a filtering stage, a whole
side-branch — can only write the `groups` section by hand, in a file, while
looking at the very nodes they want to package. This story turns packaging
into a gesture: select nodes on the canvas with 20's selection and package
them into a group; select a group instance and unpack it back into its nodes.
After it, groups are a flow in the editor — package the stage you mean, keep
working, unpack when you want to see inside again — rather than a format they
happen to round-trip through.

**The package gesture.** A package command is available for any non-empty
selection (20) — a context-menu entry on the selection, the editor's first
context menu, opened by right-click on a selected node without clearing or
reducing the selection it acts on (right-click on an unselected node selects
that node alone first, as a plain click does), with a keyboard path per 22's
pattern issuing the same operation. Any selection packages: one node
as readily as twenty (a group of one is a legitimate keep-this-intact move,
and REQ-72 says the gesture invents no special cases to police it). The
command asks for the group's name in a small dialog on 15's pattern — the one
input the gesture needs — opened pre-filled with an available suggestion
("Group", then "Group 2", ...) so Enter alone packages; Enter commits and Esc
cancels, as 22 makes every dialog.

Everything else is derived from the definition the server holds — the browser
never assembles the group by hand. The selection's nodes and the edges wholly
inside it become the group's body; each edge crossing the selection boundary
becomes an exposed port: an edge from a packaged output to a node outside
becomes an exposed output port binding that inner output (one port serving all
its external downstreams — REQ-20's fan-out preserved), and an edge from
outside into a packaged input becomes an exposed input port binding that
input. Each derived port takes the bound inner port's name — deduped across
the new port set ("out", then "out 2", ...) — and declares that inner port's
type references. The packaged nodes' stored positions move into the body's
metadata as offsets relative to the selection's centroid — a node carrying no
stored position contributes neither centroid nor offset and lands by 12's
fallback on unpack — and the new group instance sits at that centroid, where
the user's attention was, and takes the selection, so the sidebar shows it as
20 provides for any selection. The visible result: every external wire that
touched the selection now lands on the collapsed node's ports, the graph
means exactly what it meant before, and nothing moved that the user didn't
choose.

Refusals are honest and cheap: a name that duplicates a document group or
collides with a linked plugin type reference — 23's error set — is refused
naming the clash, the dialog's suggestion having already avoided the common
case; and an edge crossing the boundary at an unknown-typed placeholder node
inside the selection (12) blocks packaging with a message naming that node,
since an exposed port must declare types and a placeholder's ports declare
none. A placeholder inside the selection whose edges stay internal packages
fine — nobody asks its ports anything; a placeholder outside the selection
does not block — the exposed port declares the inner port's type references,
and the placeholder fails compile after packaging exactly as it did before.
Beyond these the gesture validates nothing (REQ-60, REQ-72): a selection with
no crossing edges still packages, into a group with no exposed ports — legal
by 23's format and inert — and enforcement stays at compile time as everywhere
else.

**The unpack gesture.** An unpack command on the group instances in the
selection — same entry points, context menu and keyboard path, one unpack
operation per instance after 20's fan-out precedent, ordinary nodes in the
selection left as they are — dissolves each back into its nodes. The body's
nodes reappear on the canvas each at its stored offset from where the instance
sat, so a package → move → unpack cycle puts the nodes back around where the
group ended up, not where they were first drawn — the user's spatial frame
survives the round trip — and the reappearing nodes take the selection the
instance held, so unpacking lands the user inside what they just opened up; a
body node without a stored position lands by 12's deterministic fallback.
External wires
re-attach through the bindings: the external upstream of an exposed input
reaches the bound inner input, the bound inner output feeds the external
downstreams, and a value typed on an unconnected exposed input (23's inline
field, whether typed there or committed through a multi-selection) lands on
the bound inner input as its parameter — the value the user gave the group's
input is the value the inner node now shows, not a lost edit. The gesture
always succeeds structurally: a returning node uuid that collides with a
surviving node is reassigned fresh rather than letting an invisible identity
refuse a visible gesture. And the group definition leaves with the instance
when no other instance references it — unpacking the last copy leaves no
ghost definition the canvas never shows — while other instances of the same
definition keep it, theirs.

Both gestures are edits like any other: single atomic operations added to 11's
catalogue, applied to the held definition and pushed whole (12's shape), so
every open tab shows the packaged group or the unpacked nodes together, never
a half-applied state, and a refusal leaves the definition exactly as it was,
the connection usable. While a run is on (16), both go quiet with every other
editing gesture — context menu and keyboard path alike — while selection
stays live. No new format, no new rendering, no new message kinds beyond the
two operations: the packaging writes exactly the shape 23 defined, the
collapsed instance renders exactly as 23 renders any group instance, and
save/reload (15) carries it by 23's verbatim round-trip.

Deliberately not here: instantiating a group definition that already exists —
a second copy of a packaged thing — is story 26 (23's reuse rationale needs a
gesture of its own); entering a group to view or edit its inside without
unpacking — unpacking is the way in, 23's settled line, an enter-group view a
future story if the loop asks; editing a group's exposed ports in place
(unpack-and-repack covers it, 23); renaming a group definition; groups in the
palette (23 settled: document-local, never registry types); any change to the
file format (23 wrote it).

## Definition of done

- With a non-empty selection on the canvas, a package command is available —
  a context-menu entry with a keyboard path issuing the same operation, the
  menu opened by right-click on a selected node without clearing or reducing
  the selection it acts on, right-click on an unselected node selecting that
  node alone first as a plain click does — and asks for the group's name in a
  small dialog pre-filled with an available suggestion, Enter committing and
  Esc cancelling; on commit the selection becomes one collapsed group
  instance rendered exactly as 23 renders group instances, sitting at the
  centroid of the packaged nodes, with the packaged nodes' stored positions
  carried into the group body as offsets relative to that centroid — a node
  carrying no stored position contributing neither centroid nor offset — and
  the new instance takes the selection. Any non-empty selection packages,
  including a single node and a selection with no crossing edges. (REQ-44,
  REQ-55, REQ-12, REQ-72)
- The packaging is derived from the held definition: edges crossing the
  selection boundary become the group's exposed ports — an edge from a
  packaged output to a node outside becomes an exposed output port binding
  that inner output and serving all its external downstreams, an edge from
  outside into a packaged input becomes an exposed input port binding that
  input, each derived port taking the bound inner port's name, deduped
  across the new port set ("out", then "out 2", ...), and declaring that
  inner port's type references — and after packaging every wire that touched
  the selection lands on the collapsed node's ports, the graph meaning
  exactly what it meant before. Internal edges of the selection, and a group
  instance inside it, travel into the body unchanged. (REQ-44, REQ-20, REQ-21)
- A refused packaging names its reason and changes nothing: a name
  duplicating a document group or colliding with a linked plugin type
  reference is refused naming the clash; an edge crossing the selection
  boundary at an unknown-typed placeholder node inside the selection blocks
  packaging naming that node, while a placeholder inside the selection whose
  edges stay internal packages fine. In both cases the definition is exactly
  as it was and the connection stays usable. (REQ-71, REQ-60)
- An unpack command on the group instances in the selection — the same
  context menu and keyboard path, one unpack operation per instance after
  20's fan-out precedent, ordinary nodes in the selection left as they are —
  dissolves each: the body's nodes reappear each at its stored offset from
  where the instance sat (a node with no stored position by the story 12
  deterministic fallback), external wires re-attach through the bindings —
  external upstream to the bound inner input, bound inner output to the
  external downstreams — a value typed on an unconnected exposed input lands
  on the bound inner input as its parameter, the reappearing nodes take the
  selection, a returning uuid that collides with a surviving node is
  reassigned rather than refusing the gesture, and the group definition is
  removed when no other instance references it and left in place otherwise.
  (REQ-44, REQ-12, REQ-71)
- Both gestures are single atomic edits applied server-side through their
  own operations in 11's catalogue style and pushed whole, per 12's shape:
  every open tab shows the packaged group or the unpacked nodes together,
  never a half-applied state, and a change made through one tab appears in
  the other. (REQ-2, REQ-45)
- While a run is on, both gestures are inert — context menu and keyboard path
  alike — while selection stays live, under the same run lock as every edit.
  (REQ-24, REQ-25)
- Keyboard operability follows 22's pattern: package and unpack each have a
  keyboard path issuing the same operation their pointer entry sends, the
  name dialog operable and escapable, Esc the quiet cancel that leaves the
  selection as it was. (REQ-44)
- Round trip through the editor's file path holds: a packaged graph saved (15)
  and reopened shows the group verbatim — name, ports, bindings, body,
  offsets — by 23's round-trip, and unpack works identically after a reopen.
  (REQ-11, REQ-12, REQ-44)
- Focused tests cover: package derivation — crossing edges to exposed ports,
  internal edges staying internal, a selection of one and one with no
  crossing edges packaging into a group with no exposed ports, centroid
  placement, relative offsets in body metadata; unpack splice — wire
  re-attachment through bindings, the exposed-input value landing on the
  bound inner input, offset placement, the definition removed with the last
  instance and kept for surviving ones, uuid collision reassignment; each
  refusal above answered with the definition untouched; the atomic push to
  every connection. UI tests (20's runner) cover the gestures end to end:
  the dialog with its suggestion, the
  collapsed node with wires landing on its ports, unpack restoring nodes and
  wires, and the run lock silencing both gestures. The workspace builds and
  passes `cargo test` and `cargo clippy` cleanly, with the UI build and UI
  test runner part of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: build a small graph in the
  editor example, select a stage that has wires crossing its boundary,
  package it, and see one collapsed node with the group's ports where the
  wires now land; run it and watch values cross the boundary as before;
  unpack it and see the nodes come back around where the group sat, wires
  intact; package again, save, reload, and see the group verbatim, then
  unpack again; try packaging under an already-used name and see the refusal
  leave the graph untouched; start a run and see both gestures do nothing
  while selecting still works.

## Comments

- 2026-09-16 — Scope seams: two new operation messages in 11's catalogue —
  package and unpack — the first additions since 20, whose fan-out rode 12's
  move, 13's delete, and 14's parameter edit rather than extending the
  catalogue, and for the reason 20's precedent cannot cover them: no
  composition of per-node operations derives exposed ports and moves
  positions atomically, and a
  half-applied packaging would be a broken graph. The server derives the
  packaging from the definition it holds, so the browser never holds a second
  copy of the groups format — 23's verbatim rule is the browser's too. All
  rendering, protocol carry, and compile behaviour are 23's, consumed
  untouched. Instantiation of existing groups is story 26; the enter-group
  view is 23's named future story; renaming and in-place port editing are
  deliberately absent (unpack-and-repack). (REQ-74)
- 2026-09-16 — UX calls settled here: the name ask opens pre-filled with an
  available suggestion so Enter alone packages — naming is the one input the
  gesture needs, and typing it should be optional; the instance sits at the
  selection's centroid and body positions are stored as offsets from it, so
  package → move → unpack keeps the user's spatial frame (absolute inner
  positions would snap unpacked nodes back to where they were first drawn —
  exactly the jump that reads as the editor losing the user's place); unpack
  of the last instance removes the definition, because a definition no
  instance shows is dead weight only a hand-writer can see (23's line,
  inverted for the gesture); a uuid collision reassigns rather than refuses,
  because refusing an unpack over an invisible identity would put a format
  detail in the gesture's way; groups of one package without ceremony
  (REQ-72).
- 2026-09-16 — Why the placeholder refusal, and why it is the only structural
  check: an exposed port must declare types (23's port vocabulary) and a
  placeholder's ports declare none, so when the placeholder is inside the
  selection its port is the one that would become the group's exposed port,
  and there is nothing honest to declare on it — this is the format's
  requirement surfacing at the gesture, not an editor-side rule (REQ-60,
  REQ-71). A placeholder outside the selection puts none of its ports on the
  boundary — the exposed port declares the packaged side's type references —
  so packaging does not police it; the placeholder fails compile after
  packaging exactly as it did before. Everywhere else packaging trusts the
  selection and lets compile judge the result, matching the editor's
  warn-early-enforce-late line.
- 2026-09-16 — Relationship to 26: this story is the lifecycle's middle —
  package what is on the canvas, dissolve what is packaged. Creating a second
  instance of a group that already exists (23's reuse rationale:
  "a packaged thing you cannot reuse is barely packaged") is a different
  gesture — nothing here creates an instance without taking its body from the
  canvas — so it is story 26's, sketched in the backlog.
- 2026-09-17 — Review calls, recorded so they hold: opening the context menu
  never disturbs the selection it acts on — the editor's first context menu,
  so 20's plain-click rule needed an explicit right-click settlement or a
  marquee selection would be destroyed at the moment the menu was opened to
  act on it, right-click on an unselected node selecting that node alone
  first as a plain click does; both gestures hand their product the selection
  (the new group instance, the reappearing nodes), because the packaged nodes
  cease to exist on commit and 20's sidebar rule would otherwise empty the
  selection and close the sidebar at the exact moment the gesture produced
  something; unpack is one unpack operation per group instance in the
  selection, ordinary nodes untouched — 20's fan-out composing for free, no
  new mechanism — rather than single-instance-only; and the placeholder
  refusal is scoped to a crossing edge whose in-selection endpoint is the
  placeholder's port, the only case with nothing honest to declare on the
  boundary, a placeholder outside the selection staying compile's to judge
  exactly as before (REQ-72).

- 2026-09-19: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
