---
title: Accessibility and density pass
date: 2026-09-16
---

## Description

Stories 12–21 built the editor surface by surface, and each parked its
accessibility work here by name: story 12 kept the canvas gestures on the
canvas library's own interaction primitives "so 22's accessibility pass
adjusts behaviour instead of replacing custom input code"; the keyboard
gestures 13 and 20 deferred; the contrast of the type channel 19 left as
"a contrast pass, not a rebuild"; the status marks 17 said "meet the floor
22 passes over"; the marks, toasts, and banner 18 handed over. The
density promises REQ-47 makes — compact nodes, only the required
information — were kept story by story, but never re-checked against the
accumulated whole. This story is that pass. The vision's quality line sets
the bar: the UI stays compact and information-dense per the Blender
reference, with a basic accessibility floor — keyboard operability, visible
focus, sufficient contrast. After it, a user who cannot use a mouse can
still build, run, and save a graph; nothing on the canvas or the chrome can
disappear into the background; and the surfaces 12–21 delivered are still
the compact ones they promised to be.

The floor's largest piece is keyboard operability. Every editing
capability the editor offers gains a keyboard path alongside its pointer
gesture — pan and zoom stay pointer-only, focus being the keyboard's way
around the canvas — the pointer gestures stay exactly as they are, and the
keyboard paths issue the same operations, the precedent story 20 set with
its per-node fan-out. Concretely: every chrome control — palette, file
controls, start/stop — is reachable and operable by keyboard, and the
dialogs 15 added (the save-path ask, the unsaved-changes confirm) are
operable and escapable; the canvas is traversable — nodes focusable,
selection built and cleared mirroring 20's
selection model; an activated palette row creates a node at a
deterministic, visible position through the same create operation a drop
sends; a focused node moves by arrow key, committing through the same move
operation; wiring and unhooking have keyboard paths issuing the same wire
and unhook operations, replace semantics included; Esc is the keyboard's
quiet cancel — clearing the selection (so the sidebar closes by 14's rule),
standing down an in-progress keyboard wire, discarding an uncommitted field
edit as it already does. Delete keeps the canvas-focus scoping 20 gave it:
with a field focused the same keys edit text and never delete. While a run
is on, every keyboard editing path goes quiet with its pointer twin — the
lock is one rule over both input modes, not two. The exact keys are a
development choice, recorded in this story's comments; the constraint the
floor sets is that a path exists for every editing gesture, and that the
keys are conventional enough to guess.

Visible focus: every focusable element shows a clearly visible focus
indicator — chrome controls, palette rows, the sidebar's fields, the
inline fields on nodes, the nodes and ports themselves on the canvas. Focus
order runs sensibly through chrome, canvas, sidebar; nothing traps focus or
leaves it invisible; and moving focus commits nothing on its own — a field
nothing was typed into commits nothing when focus leaves it, so tabbing
through the sidebar unsets nothing — while 14's Enter-or-leave commit for a
typed edit and 20's mixed-field scoping stand. And so the editor's
no-silent-states promise holds for everyone: 18's toasts and the
disconnected banner are announced to assistive technology as they appear,
without stealing focus; the durable truth stays on the canvas and in the
chrome where 18 put it.

Sufficient contrast, judged against the light Blueprint chrome the editor
ships in (REQ-50): text meets a 4.5:1 ratio against what it sits on, and
large text and meaningful edges — port dots, wires, status and problem
marks, focus indicators, the banner — 3:1. The editor's own colours are the
editor's to fix: chrome, canvas, the neutral port-and-wire pair, the shape
set, the marks, the banner, the toasts. The base scalars' port colours are
core's own declarations through the same mechanism a plugin uses, so where
one falls short the fix is a declaration change in core flowing through
19's listing fact — not a browser-side override, which would be a second
colour source. A plugin's declared colour that fails the bar is the
declarer's to fix, exactly as a malformed icon is (19); meanwhile the shape
always sits beside the colour (19), so the type information survives any
shortfall while the declaration is fixed. Plugin-supplied custom UI (21) is
plugin territory — the floor binds the editor's own rendering, the default
node class included.

The animation 17 built honours the platform's reduced-motion preference —
17 settled that call, and this pass makes it real and keeps it honest: with
reduced motion requested the pulses rest, and the derived truth still
updates — statuses and latest values are state, not motion.

Then the other half of the title. The stories since 12 added icons, value
readouts, status and problem marks, mixed markers — exactly how
compactness erodes: never in one change, always in the sum. This pass
re-checks every surface against REQ-47 and the Blender reference (REQ-49):
a node still shows only what it needs — label and icon, ports with their
inline fields or connected indicators, status and problem marks, 17's
per-port value readouts while they persist, and 20's mixed-state marker
where a multi-selected field is mixed — with declared types still living in
tooltips only, and no surface grown permanent padding or redundant text;
palette rows stay single-line; the sidebar carries only what is editable
beside 14's read-only type-reference-and-uuid orientation line; whatever
the sum accreted beyond the required information goes.

Deliberately not here: groups (23); the fizzbuzz UI mode (24); plugin-owned
custom UI, whose floor is its declarer's — a division this story sets, for
21's population to record; a dark theme or any second look — REQ-50's
light look is the one the contrast work protects; new
settings or an accessibility preferences panel — the floor is met by
default or not at all; screen-reader certification beyond announcing the
reports and states the editor already shows; new protocol messages or any
new server behaviour — the whole pass is view-side and declaration-side,
the server neither knows nor cares.

## Definition of done

- Every chrome control — the palette, the file controls, start/stop — is
  operable by keyboard alone: reachable in a sensible order, activated with
  conventional keys, with the dialogs 15 added (the save-path ask, the
  unsaved-changes confirm) fully operable and escapable. Every canvas
  gesture — create, select, build and clear a multi-selection, move, wire,
  unhook, delete — has a keyboard path issuing the same operation its
  pointer gesture sends, a keyboard-created node landing at a
  deterministic, visible position as a drop does, with Esc the quiet cancel
  (selection cleared, an in-progress keyboard wire stood down, an
  uncommitted field edit discarded), no pointer gesture changed, and pan
  and zoom stay pointer-only — the keyboard's way around the canvas is
  focus, the view following focus so every node stays reachable. The keys
  are conventional — arrows move, Enter activates, Esc cancels, Tab
  traverses. (REQ-46, REQ-56, REQ-59, REQ-2)
- While a run is on, the keyboard editing paths go quiet exactly with their
  pointer twins, by the same one lock, while keyboard navigation and
  selection stay live; while disconnected (18) they are inert like every
  server-acting gesture. (REQ-25, REQ-24)
- Every focusable element shows a clearly visible focus indicator — chrome
  controls, palette rows, sidebar and inline fields, nodes and ports on the
  canvas; focus order runs chrome → canvas → sidebar sensibly; nothing
  traps or hides focus; moving focus commits nothing on its own — a field
  nothing was typed into stays uncommitted when focus leaves it, while a
  field left with typing still commits by 14's Enter-or-leave rule; and
  18's toasts and the disconnected banner are announced to assistive
  technology as they appear, through a polite live region the UI tests
  assert — present and polite, never focus-taking. (REQ-71)
- Contrast meets the floor everywhere the editor renders: text 4.5:1,
  large text and meaningful edges — port dots, wires, status and problem
  marks, focus indicators, the banner — 3:1, all within the light Blueprint
  theme. The editor's own colours pass as rendered, and the base scalars'
  declared colours pass against the surfaces they render on — wires against
  the canvas, port dots against the node's surface — fixed by declaration
  where any fall short, the fix riding 19's listing fact with no
  browser-side override — and the neutral pair passes. (REQ-50, REQ-48,
  REQ-29)
- With the platform's reduced-motion preference requested, the value pulses
  rest while statuses and latest values still update — the derived truth is
  state, not motion, per 17's settled call.
- The density pass holds every surface to the Blender reference: nodes show
  only the required information — label and icon, ports with inline fields
  or connected indicators, status and problem marks, 17's per-port value
  readouts while they persist, and 20's mixed-state marker where a field is
  mixed — with declared types still in tooltips only; palette rows
  single-line; the sidebar only what is editable beside 14's read-only
  type-reference-and-uuid orientation line; anything the stories' sum
  accreted beyond that is removed. (REQ-47, REQ-49, REQ-70)
- Focused tests cover what the UI test runner 20 added reaches: the
  keyboard paths issuing the same operations as their pointer gestures —
  create, move, wire, unhook, delete, and the selection model — Esc's
  cancels, the run lock silencing keyboard editing, the polite live region
  asserted present and polite and never focus-taking, and the contrast
  checks — the editor's own colours and the neutral pair against the theme
  as the UI defines it, the base scalars' declared colours read from the
  listing fact against the surfaces they render on — computed in the UI
  test, no colour value copied between the Rust and browser sides. The
  workspace builds and passes `cargo test` and `cargo clippy` cleanly,
  with the UI build part of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: with the mouse unused, build a
  small working graph from the keyboard — create nodes from the palette,
  wire them, type a value into a field, rename a node, delete a deliberate
  mistake, press Start, watch it run, press Stop, save to a path and see
  the save-path ask appear; Tab through the editor and watch a visible focus
  indicator track onto nodes and ports; set reduced motion, start another
  run, and see the wires stand still while values still tick and statuses
  still turn; and read the shipped sample at arm's length — every port,
  wire, mark, and piece of text legible on the light canvas.

## Comments

- 2026-09-16 — Scope seams: no additions to story 11's catalogue and no new
  server behaviour — the keyboard paths send the operations 12–14 defined,
  and the contrast fixes ride 19's listing fact from declaration changes.
  Story 12's settled decision that the canvas gestures sit on the canvas
  library's own interaction primitives is what makes this a pass rather
  than a rebuild: the keyboard paths and focus work adjust that layer, they
  do not replace bespoke input code. The surfaces the floor covers are 12's
  shell and empty states, 13's wiring, 14's fields, 15's files and dialogs,
  16's run control and lock, 17's marks, values, and pulses, 18's toasts,
  marks, and banner, 19's icons and type channel, 20's selection model;
  21's plugin-owned rendering is exempt — the declarer's floor, like its
  colours. Groups (23) and the fizzbuzz UI mode (24) land after and follow
  the patterns this pass sets. (REQ-74)
- 2026-09-16 — Why keyboard wiring is in scope for a "basic" floor: the
  vision's floor is keyboard operability, and a keyboard-only user who can
  do everything except the editor's central gesture has not got a floor,
  they have got a patch. The paths reuse the existing operations
  one-for-one — one view-side affordance per gesture, not a second
  mechanism (REQ-2), exactly 20's fan-out precedent — so the cost stays
  pass-sized. The exact keys are settled during development and recorded
  here when they are; the constraint this story sets is convention, so the
  paths are guessable rather than collected into a help screen that would
  itself be a feature.
- 2026-09-16 — Contrast ownership, so the pass does not grow a second
  colour source: the editor's own colours are editor CSS; the base scalars'
  port colours are core's declarations through the same mechanism a plugin
  uses (19's settlement), so a shortfall is a declaration change flowing
  through the listing fact — and because the colours reach the browser as
  data, the contrast checks compute in the UI test, reading the
  declarations from the listing fact against the background the CSS
  defines, no colour value copied into Rust; a plugin's declared colour is
  the declarer's to fix, as with a malformed icon, and 19's
  shape-beside-colour rule means the type information never rides contrast
  alone meanwhile. (REQ-73)
- 2026-09-16 — The accessibility floor has no REQ id: it is the vision's
  quality principle, not a line in requirements.md, so the criteria above
  cite the requirements they do touch and stand on the vision for the rest.
  Nothing in requirements.md contradicts the floor, and REQ-50's light
  theme is the contrast work's setting, not its casualty — the pass keeps
  the look, it makes it readable.
- 2026-09-16 — The announcements reach slightly past the named floor on
  purpose: 18's whole point is that no report is ever again swallowed, and
  a report only sighted users receive is swallowed for everyone else; a
  polite live region is the smallest honest completion of that promise.
  Screen-reader behaviour beyond announcing the reports and states the
  editor already shows is not certification this story gives.
