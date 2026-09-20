---
title: No node UUIDs in the editor or its messages
date: 2026-09-20
---

## Description

Node instances are identified by uuid internally (REQ-41), and that internal
address leaks everywhere a user looks: the sidebar prints it as a fact row
(crates/nodetool/ui/src/sidebar.jsx:44-47), compile problems name nodes as
`counter (3b60e6f3-…)` (`node_name`, crates/nodetool/src/compile.rs:746), run
failures arrive as `node <label> (<uuid>): …` (crates/nodetool/src/engine.rs:454),
and server operation errors answer `no node 3b60e6f3-… in the definition`
(crates/nodetool/src/server/mod.rs:394). To a user a uuid is noise — they
named their nodes; the editor should speak the names.

Every user-visible surface — error and warning text, run failure reports,
toasts, status lines, the sidebar, and every wire's accessible name — names
nodes by their label, falling back to the type's label when the instance has
none. Labels are not unique: two nodes may share one, and where text alone
must tell them apart it cannot — a terminal's failure line reads `node
counter: …` and a message naming two feeders prints `counter` twice; in the
editor a mark points at the node the text means, so the ambiguity never
reaches the user there. The uuid stays what REQ-41 makes it: the internal
identity of wire messages, map keys, and marks — structure, never rendered
text. Of today's uuid printers, one is deliberate: core's printing observer
(crates/nodetool/src/engine.rs:639-647), a terminal timeline for embedders
that adds the uuid only where two nodes share a label — a terminal's
disambiguation, not the editor's speech — and it stays as is; no debug view
is built in the editor here.

## Definition of done

- No editor surface renders a node instance's uuid as text: the sidebar's
  uuid row is gone, and node marks, toasts, the status line, and run failure
  reports carry only labels and type names, and every wire's accessible name
  names its two nodes by label and port, not the uuids React Flow announces
  by default (REQ-40, REQ-47).
- Compile errors and warnings name nodes by label or type label: the unknown
  type, missing port, duplicate connection, cycle, and family messages read
  as `counter instantiates …`, ``input `count` of node by three …`` — no
  parenthesised uuids (REQ-40, REQ-41).
- Server operation errors that refer to an existing node name it by label; an
  error about a reference that names no node ("no such node in the
  definition") says so without printing the uuid it was handed (REQ-40,
  REQ-41). One message keeps its uuid: the duplicate-naming complaint (`node
  … is named twice in the selection`) — its subject is the duplicated
  reference the client sent, not a node, so it keeps the uuid, exactly as the
  loader's duplicate-uuid case does.
- The run failure's message and the failed node's identity stay separable —
  the failure still carries the node's uuid as structure for the mark on the
  canvas, only the *text* is uuid-free (REQ-43).
- The engine's, compiler's, and server's tests assert message text by label,
  and a hand-written YAML graph with duplicate or dangling uuids still
  produces errors a user can act on by the names in their file. A uuid is
  quoted in message text only when it is the complaint's subject or the
  referent's only name in that file — the user's own text, not an internal
  address — never as decoration beside a name (the printing observer's
  terminal timeline excepted, per its recorded decision); the loader's
  cross-check messages included: they keep their YAML-path pointers, name the
  nodes they know by label, else by the type reference the file writes (the
  group's name where the type is a group in the document), and may quote the
  uuid the file itself carries (REQ-71).
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and the
  UI test suite pass.

## Comments

- 2026-09-20 — Core's printing observer (engine.rs:639-647) adds the uuid
  only where two nodes share a label: a terminal timeline's disambiguation,
  not the editor's speech. It stays as is.
- 2026-09-20 — The fix is wider than one function. `node_name`
  (crates/nodetool/src/compile.rs:746) drops the parenthetical, but uuids
  reach user-visible text well past compile's complaints: the group-compile
  errors (compile/flatten.rs:245, 273, 324, 349, 493), the engine's
  run-failure lines (engine.rs:213, 454, 509), the server's set-parameter
  error (server/mod.rs:438), the packaging module's own dialect (`named`,
  server/packaging.rs:482, feeding the messages at 68 and 148) beside its
  uuid answers (packaging.rs:47, 50, 139), and the file loader's cross-check
  messages (graph.rs:735-771). Compile's own outliers inline uuids too: the
  duplicate-uuid complaint (compile.rs:163), the dangling-edge complaint
  (compile.rs:199-206), the duplicate-connection message naming its two
  feeders by uuid (compile.rs:242-245), and the family-wait message
  (compile.rs:857). Both display formatters — `node_name` and `named` —
  speak one rule after this story: the instance's label, else the type's
  label, else the type reference for a type nothing declares. The rule lives
  in one helper both call — a few lines on `NodeInstance` taking the type's
  label when the caller knows it (`node_name` passes the resolved type's,
  `named` the registry's) — and the parentheticals go; every later message
  site calls that one helper, so the speech cannot drift between compile and
  the server. `named` needs the registry to speak the type's label, so
  `unpack` (server/packaging.rs:134) takes one too; the straight
  type-reference fallback remains only for a type nothing declares. Each
  message names what it has: labels for defined nodes, the type's label for
  unlabelled ones; the loader keeps its YAML path pointer, compile names the
  edge by the endpoint
  that exists and its ports, saying which side is undefined. For a dangling
  reference — a node that does not exist — the message also keeps the uuid
  the file itself writes for the missing endpoint: an undefined node has no
  label anywhere, that uuid is the only name the user's file gives it, and
  it is an address in their own document, not internal noise. Compile's
  dangling-edge branch is defense-in-depth for embedders — the loader
  rejects such a file first (graph.rs:744-751) and the server's operations
  keep every edge's endpoints in the definition (delete_node drops its
  edges) — so the uuid it quotes is the user's own file content, not leaked
  internals. The printing observer's timeline name (engine.rs:641) keeps its
  colliding-label disambiguation, per the decision above. Grep the format
  strings for uuids; the DoD is the rule, not this list.
- 2026-09-20 — A duplicate-uuid error is *about* a uuid collision. When the
  two instances' labels differ, name both by label and say they share an
  identity; when the labels read the same — the default, two unlabelled
  counters both read `counter` — the labels cannot point at the nodes, and
  the colliding uuid is the only search key the file offers, so the message
  names the identity itself: ``the nodes `counter` and `counter` claim the
  same identity 3b60e6f3-…``.
- 2026-09-20 — Wire messages keep uuids as keys — that is the protocol's
  structure, addressed by REQ-41 and untouched. The line is rendering: if it
  becomes visible text in the browser, it is a label; if it stays a field in
  a JSON envelope or a Map key in the UI, it is an address.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
