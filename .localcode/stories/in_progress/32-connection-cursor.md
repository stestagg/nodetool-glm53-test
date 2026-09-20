---
title: Connection cursor in the port hot-zone
date: 2026-09-20
---

## Description

Wires are made and unmade by dragging on the port handles (REQ-54), but the
cursor a handle shows under the pointer is the wrong one: the app paints the
handles `grab` (crates/nodetool/ui/src/style.css:436) — the hand of a node
drag, the cursor the node title and the palette item also wear — and, later
in the bundle than React Flow's own sheet, it talks over the `crosshair`
React Flow already gives connectable handles
(`.react-flow__handle.connectionindicator`). The hot-zone is visible but
lying: `grab` invites a node drag where a wire gesture can start, land, or
be unhooked — an invitation the node's title bar, not the port, can honour.
The pointer should say what the surface under it can do.

While editing is unlocked, hovering a port handle shows a connection
cursor (`crosshair`) — the handle is the hot-zone, and the cursor says a
connection can start or be unhooked here. While a wire drag is in flight,
the connection cursor holds over the drag's origin and over the handles
the wire can land on — the landing zone stays legible mid-gesture — and an
editor whose editing is locked keeps today's quiet default cursor on
handles, as it already does for the other gestures (style.css:127-134).

## Definition of done

- Hovering a port handle in an unlocked editor shows a connection cursor —
  `crosshair` or equivalent — visibly distinct from the `grab` the pannable
  canvas and the draggable surfaces (titles, palette) show, so the hot-zone
  reads as its own gesture (REQ-54).
- While a wire drag is in flight, the connection cursor holds over the
  drag's origin and over the handles the wire can land on, so the landing
  zone stays legible mid-gesture (REQ-54).
- An editor with editing locked — a run in progress (REQ-25), or a lost
  connection — keeps the default cursor on handles: the invitation to drag
  what cannot be dragged is not shown.
- Panning, node dragging, and every other existing cursor are unchanged —
  the palette item's and node title's `grab`, the chrome's `pointer` — and
  the lock keeps its override on handles; the UI test suite and
  `cargo test --workspace` pass.

## Comments

- 2026-09-20 — Small and contained, and mostly a deletion: @xyflow/react 12
  already ships the connection cursor — `.react-flow__handle
  .connectionindicator` in its own stylesheet gives connectable handles
  `pointer-events: all` and `cursor: crosshair` — and the app's handle rule
  (style.css:430-437) talks over it, the app sheet imported after the
  library's (main.jsx:4-5). The lock does not remove that class: these
  `Handle`s never receive `isConnectable` (nodes.jsx:110, nodes.jsx:171),
  so the library's connectable default stands and the `.app.locked` rule
  (style.css:127-134) is what keeps the default cursor on locked handles —
  the rule the third criterion is a regression guard on. Mid-drag the
  version in use exposes no wrapper or body class: the drag's own handle
  carries `connectingfrom`, the handles the wire can land on keep
  `connectionindicator`, and handles it cannot land on are
  pointer-transparent — the right silence, no rule for them. The expected
  change is deleting the `cursor: grab` line at style.css:436 and
  verifying: hover and the landable handles then wear the library's
  crosshair. One hole is known up front — the library gives the drag's
  origin (`connectingfrom`) pointer events but no cursor — so cover it
  with one small rule only if verification shows it; no new state machine.
- 2026-09-20 — Keyboard wires land on the same handles; the cursor is a
  pointer affordance only and says nothing about them — no aria or focus
  change rides along.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
