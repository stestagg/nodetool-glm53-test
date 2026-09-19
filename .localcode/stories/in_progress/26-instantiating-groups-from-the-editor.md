---
title: Instantiating groups from the editor
date: 2026-09-16
pr_id: 63
depends: [25]
---

## Description

Story 23 made a group exist — a named definition in the document whose
instances collapse to one node and run — and story 25 made it a flow: package
what is on the canvas, unpack what is packaged. What nothing yet lets the
user *do* is reuse. A group definition sits in the document; instances of it
sit on the canvas; and the only ways to get a second instance are to package
a second set of nodes or hand-write one into the file. The user who packaged
a filtering stage and needs the same stage in this graph's other branch
should drop in another copy the way they drop in any node — not re-derive it.
23's reuse rationale ("a packaged thing you cannot reuse is barely packaged")
becomes a gesture too rather than a hand-written file.

**Where the gesture lives.** Groups are document-local, never registry
types, so they take no palette entry — 23 settled that. The surface they do
get is the canvas's context menu: 25 introduced the menu on the selection;
this story gives the canvas background its own — a Groups submenu listing the
document's groups by name, alphabetically — 19's deterministic order, so
every tab and every reload shows the same menu — derived from the definition
the browser already holds (23's verbatim rule is the browser's too). Picking
a name creates one instance of that group, landing where the menu was
opened — where the user's attention is, 12's drop-point precedent. With no
groups in the document the background menu itself does not open — its one
content is this submenu, and an empty menu is the noise the absent row
avoids, one level up. The submenu tracks the definition as it changes:
packaging a
selection (25) puts the new group in the menu at once; unpacking a
definition's last instance (25) takes it back out. A group with zero
instances — the hand-writer's dead weight, legal by 23 — is exactly what the
menu makes reachable: open a hand-written file whose group no instance
references and instantiate it from the editor, instead of hand-editing the
file again just to use what the file already defines.

**The mechanism is the one create already is.** No new message kind: the
create operation (12) names a type reference and a position, and 23 already
settled that a group instance's type reference resolves against the
document's groups before the node-type listing's. The one widening this
story makes server-side: create, which today refuses any type reference the
listing does not provide, resolves a document group's reference first, per
23's order — an instance of a group is an ordinary node instance, so its
creation is the ordinary create. The server assigns the instance uuid
(REQ-41), records the position in the node's metadata (REQ-12), applies, and
pushes whole (12's shape) — every open tab shows the new instance together.
The new instance renders exactly as 23 renders any group instance: the
group's exposed ports, the group's name as its default label, overridable
like any node's (REQ-40) — two copies of a group label alike exactly as two
Counters do, and the label override is the disambiguator. A create naming a
group the document does not define — a stale tab after an unpack took the
definition away — is answered with an error naming the problem, the
definition untouched, the connection usable, 11's framing; beyond that
nothing is checked at edit time (REQ-60, REQ-72): types and cycles are
compile's judgement, as for every other instance.

Keyboard operability rides 22's floor and its precedent: the context menu
and its submenu are operable and escapable by keyboard, and activating a
group's row creates the instance at a deterministic, visible position
through the same operation a pointer activation sends — the palette-row
rule, applied to the document's groups. While a run is on (16), the entry
goes quiet with every other editing gesture, pointer and keyboard alike,
while selection and navigation stay live. Nothing else moves: no format
change (23 wrote it), no rendering change (23 renders it), and save/reload
(15) carries the new instance by 23's verbatim round-trip.

Deliberately not here: entering a group to view or edit its inside without
unpacking — unpacking is the way in, 23's settled line; a second entry point
on an existing instance's menu ("add another copy of this group") — the
background menu covers that case and the zero-instance one with one surface;
renaming a group definition (25's unpack-and-repack covers it); groups in
the palette (23); copy-paste of instances (20's line); any change to the
file format (23 wrote it).

## Definition of done

- With one or more groups defined in the document, the canvas background's
  context menu carries a Groups submenu listing them by name, alphabetically,
  derived from the definition the browser holds; picking a group creates one
  instance of it landing where the menu was opened, the server assigning the
  instance uuid and recording the position in the node's metadata; with no
  groups defined the background menu itself does not open, and the submenu
  tracks the definition live — packaging a selection adds its group to the
  menu, unpacking a definition's last instance removes it. (REQ-44, REQ-45,
  REQ-41, REQ-12)
- The instantiation rides the ordinary create operation: a group instance's
  type reference resolves against the document's groups before the
  node-type listing's (23's order), so create accepts a document group's
  reference where previously it refused any reference the listing did not
  provide; the applied instance is an ordinary group instance rendered
  exactly as 23 renders group instances — the group's exposed ports, the
  group's name as default label, user-overridable — and the change is
  pushed whole, every open tab showing the new instance together, a change
  made through one tab appearing in the other. (REQ-44, REQ-45, REQ-2,
  REQ-40)
- A create naming a group the document does not define is answered with an
  error naming the problem, the definition untouched, the connection
  usable; nothing else is validated at edit time — an instance that cannot
  compile lands freely and compile judges when a run starts, as for every
  other node. (REQ-71, REQ-60, REQ-72)
- The keyboard path issues the same operation: the context menu and its
  submenu operable and escapable by keyboard, activating a group's row
  creating the instance at a deterministic, visible position, Esc the quiet
  cancel that closes the menu creating nothing. (REQ-44)
- While a run is on, the gesture is inert — pointer and keyboard paths
  alike — under the same run lock as every edit, while navigation and
  selection stay live. (REQ-24, REQ-25)
- Round trip through the editor's file path holds: an instantiated group
  saved (15) and reopened shows the instance verbatim — an ordinary node
  instance in 23's format — and instantiation works identically after a
  reopen, including on a hand-written file whose group no instance
  references. (REQ-11, REQ-44)
- Focused tests cover: create with a document group reference — uuid
  assigned, position recorded in metadata, applied to the held definition
  and pushed to every connection; create with a reference a hand-written
  document's group and a linked plugin type both provide is accepted — no
  ambiguity refusal invented at edit time (REQ-72) — the group-before-listing
  order showing only where it is observable, in the browser's composition
  per 23; create with an unknown group reference refused with the definition
  untouched. UI tests (20's runner) cover the submenu derived from the held
  definition and tracking it — a package adding its group, an unpack of the
  last instance removing it — the instance landing at the opened menu's
  position, keyboard activation at a deterministic position, no entry with
  no groups, and the run lock silencing the gesture. The workspace builds
  and passes `cargo test` and `cargo clippy` cleanly, with the UI build and
  UI test runner part of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: build a small graph, package a
  stage (25), right-click the canvas background elsewhere, and pick the
  group from the Groups submenu — a second collapsed instance lands where
  you clicked, ports and default label the group's; wire it in, run, and
  see both copies working; unpack one and see the other keep its group;
  instantiate, save, reload, and see both instances verbatim; open a
  hand-written file whose group has no instance and instantiate it from the
  menu; start a run and see the entry do nothing while panning still works.

## Comments

- 2026-09-16 — Scope seams: no new message kind — the one server-side
  change is create's resolution widening to 23's groups-before-listing
  order, the first change to 12's create since it landed, and it is a
  widening, not a branch: the server still applies one create operation by
  type reference (REQ-2, REQ-73). Rendering, protocol carry, compile, and
  the file format are 23's, consumed untouched; the context menu and the
  run lock are 25's and 16's patterns, extended not reworked. This story
  completes the group lifecycle — package (25), instantiate (26), unpack
  (25) — and no later story touches groups unless the packaged-up loop asks
  for the enter-group view. (REQ-74)
- 2026-09-16 — UX calls settled here: the background context menu rather
  than a palette section, because 23 settled that groups take no palette
  entry — they arrive with the document, not the registry — and a
  document-shaped list wants a document-shaped surface; the background menu
  itself does not open when no groups exist, since a menu row that cannot
  act is noise and the menu's one content is that row — an empty menu is the
  same noise one level up; the palette's empty message is the different
  case, a primary surface that would otherwise sit silently blank; one
  entry point only, with no "add another" on an instance's menu, because
  the instance-menu case is strictly contained by the background menu's and
  a second route in is a second thing to keep in mind; two copies of a
  group label alike, exactly as two nodes of one type do — the label
  override (REQ-40) is the disambiguator, and numbering copies would be the
  editor inventing names the user did not choose; the instance lands at the
  opened menu's pointer position, 12's where-the-attention-was rule, with
  the keyboard path taking 22's deterministic-position precedent because a
  keyboard user has no pointer to place.
- 2026-09-16 — Why the zero-instance case drives the surface: 23 made "a
  group no instance references" legal — dead weight is the hand-writer's to
  see, not a failure mode — but in the editor such a group is invisible and,
  without this menu, unusable; the background menu is the one surface that
  reaches it, which makes it also the door by which a hand-written file's
  groups become usable in the editor without a second hand-edit. The
  editor-only flow never meets the case — 25 removes a definition with its
  last instance — but the headless-to-editor flow meets it first thing.
- 2026-09-17 — Review settlements: the submenu lists the document's groups
  alphabetically, 19's deterministic order for palette rows, so every tab
  and every reload shows the same menu and the UI test has an order to
  assert; and the name-collision test asserts acceptance rather than the
  server's resolution order — a reference a document's group and a linked
  plugin type both provide is accepted under any order, the applied
  instance the same ordinary node instance either way, carrying no
  classification and no echoed resolution — so the test pins only what
  create actually decides, the membership check plus its refusal, no
  ambiguity refusal invented at edit time (REQ-72), the group-before-listing
  order remaining the browser's composition rule per 23, tested where it is
  observable. The collision state's only producer is a hand-written
  document — 25's packaging refuses to create it, 23's compile flags it at
  the next run, and nothing removes it in between — so the hand-written
  file is the fixture.
