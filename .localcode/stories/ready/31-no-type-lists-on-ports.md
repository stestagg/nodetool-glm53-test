---
title: Never show a list of types on a port
date: 2026-09-20
---

## Description

Ports can declare a union of types (REQ-27), and the editor renders that
declaration verbatim: a connected input on the node prints
`port.type_refs.join(', ')` as its "connected" indicator
(crates/nodetool/ui/src/nodes.jsx:126), the same join rides the port tooltip
(nodes.jsx:108, nodes.jsx:170), and the sidebar prints
`connected — i8, i16, i32, …` (crates/nodetool/ui/src/sidebar.jsx:57). On a
numeric-family port that is a wall of ten type names where the node needs
one word or none — space is at a premium (REQ-47), and a list of types is
neither the type the port carries nor information the user asked for.

A port never renders a list of multiple types. A port declaring exactly one
type shows that one name, whether or not the listing has a fact for it —
the declared reference is the graph's own fact, and a missing listing fact
silences the dot, not the text. Only a declaration of more than one type —
a union, a family — shows no type text at all: the tooltip stays empty of
type noise rather than filling with a list.

When a connection replaces a scalar field, what fills the slot is the
connection's own indicator, not a stand-in for a type: the span on the node
and the row in the sidebar read `connected` as visible text — not only a
`title` — with no type names appended (REQ-58). A single-type port's
declared name keeps living in the tooltip, where the node has always kept
its declared types.

## Definition of done

- No port, on a node, in a tooltip, or in the sidebar, renders more than one
  type name; a union-declared or family port shows no type text (REQ-47).
- A connected input's indicator on the node and in the sidebar reads
  `connected` — visible text, not only a `title` — with no type names
  appended, and the span's own `connected` tooltip goes with it, leaving
  the port row's declared-name tooltip the one answer a hover gives
  (REQ-58).
- A single-type port's dot and tooltip keep their current behaviour — the
  dot typed from the listing's fact, neutral when the listing has none, and
  at most that one name in the tooltip — while its wired indicator reads
  `connected` as the criterion above says, single type included (REQ-27,
  REQ-48).
- The node and sidebar tests together cover a union port, a single-type
  port, and an unknown-type reference — the unknown reference living in the
  node tests, since the sidebar renders only scalar-possible ports
  (fields.jsx:41) — asserting no type text for the union, the declared name
  for the single-type and unknown-reference cases, and the word `connected`
  on a wired input, the word asserted, not only the list's absence.
- The UI test suite, `cargo test --workspace`, and `cargo clippy --workspace
  --all-targets` pass.

## Comments

- 2026-09-20 — The resolved type is deliberately out of scope: the true
  per-connection type is a compile-time fact (REQ-28), and the editor works
  on the definition while it is edited — wiring the compile's resolutions
  back into the live view is a bigger story about the server's state, not
  this one. Until that exists, hide: REQ-58 asks for a connected/type
  indicator, not for the types' names.
- 2026-09-20 — The four call sites are the whole surface: the tooltip
  `title` on both port divs (nodes.jsx:108, nodes.jsx:170), the
  `.port-connected` span (nodes.jsx:126), and the sidebar's connected row
  (sidebar.jsx:57). `portAppearance` (types.js:23) already draws the
  single-versus-union line for the dot; the text follows the same rule,
  stated once beside it in types.js — one derivation returning a name or
  null, read by every site that shows or hides type text rather than
  re-derived inline at each; the two value-UI lookups (nodes.jsx:148,
  pluginui.jsx:124) read the same derivation — the same classification,
  asked for a different use — and the derivation is named for what it
  answers (the port's one declared reference, or none), not only for the
  text it renders. One place the resolved-type story of comment 1 extends,
  not the last two inline copies of the line for it to hunt.
- 2026-09-20 — A tooltip is rendered text; it is in scope. One name is
  compliant, a join is not. The port's declared family name does not travel
  the protocol — `ports_json` (crates/nodetool/src/server/protocol.rs:157)
  sends `name` and `type_refs` only, and the per-reference fact is colour,
  shape, and ui — so a family port has no single word to show and its
  tooltip is empty. Adding a family word is a listing change, server work
  this story does not count; a family display name, if ever wanted, is a
  protocol story of its own.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
