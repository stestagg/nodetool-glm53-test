---
title: Compiler to executable graph
date: 2026-09-16
---

## Description

A graph user hands over a definition — hand-written YAML today, editor-built
later — and this is the moment it either becomes runnable or comes back with
an understandable list of what is wrong. Compilation is a pure, fast
transformation from story 03's graph definition, together with the node and
type registries from stories 01–02, to one immutable compiled graph: the plan
story 06 wires streams from, with every decision already made.

Validate what correctness requires, no more: every node's type reference
resolves, every edge names ports that exist on those types, no input is fed
twice, the graph is a DAG, and every input is either connected or carries a
value — an input with neither would wait forever and hang a run silently, so
it is caught here, by name. Then resolve: each connection's concrete type is
negotiated across unions, with declared trivial conversions bridging gaps no
direct overlap covers; each stored parameter is materialised into a typed
runtime value (story 02's), custom types through their registered ser/de
hooks. What comes out carries, per node instance uuid: the resolved node
type, a resolved type per port, and per input either its upstream (with any
conversion to apply) or its constant — the engine routes by instance and
never needs the registries again.

The open question story 03 deferred is settled here: an applied conversion is
recorded in the compiled graph — it must exist there, as the adapter the
runtime applies — and stays implicit in the file. A file that recorded its
own conversions would go stale whenever registries change; deriving them by
recompiling keeps files honest, and recompiles are cheap by design.

Enforcement lives exactly here: late enough that free editing is never
blocked — the editor warns early by recompiling (story 18) — early enough
that a run never starts on a broken graph. Errors are collected, not
fail-fast: one pass reports every independent problem, each naming the node
by the label the user set or its uuid, the port, and what is wrong — a list
the editor can show as-is.

## Definition of done

- Compiling a valid definition yields an immutable compiled graph recording,
  per node instance uuid, the resolved node type, a resolved type for every
  port, and per input either its upstream edge — with any conversion adapter —
  or its typed constant: everything story 06 needs to wire streams with no
  further registry lookups or type decisions, and nothing that branches on
  what a node is called or which type it is. (REQ-15, REQ-41, REQ-73)
- Compile is a pure function of definition and registries — same inputs, same
  result, no side effects — and fast enough to sit inside the editor's
  warn-early loop and the stop–recompile–restart path without being noticed.
  (REQ-26, REQ-60)
- A definition with structural problems compiles to no graph and an error
  list covering every independent problem in one pass, each message naming
  the node (user label where set, else uuid), the port, and the fault:
  unknown node type reference; an edge naming a port the resolved type does
  not have; a second edge into an already-fed input; a parameter keyed to no
  input port; a cycle, reported as the node path that forms it; an input with
  neither connection nor value. Unconsumed outputs are fine. Nothing is
  checked beyond what correct execution needs, and a broken file never
  panics. (REQ-1, REQ-21, REQ-71, REQ-72)
- Each connection's type is resolved at compile time: from the overlap of the
  output's and the input's accepted types; where the overlap is empty, a
  single declared trivial conversion bridges it and the adapter is recorded —
  i32 into an f64-only input just works, i64→f64 never does; where more than
  one declared conversion bridges, that is an error naming the candidates
  rather than a silent pick that changes the user's data; where nothing
  bridges, the error names both ends' types. A genuine overlap stays a union:
  the downstream input accepts every member, values arrive tagged, and no
  member is pinned arbitrarily. No conversion to string is ever inserted —
  turning values into text is an explicit Format node's job. (REQ-27, REQ-28,
  REQ-35, REQ-36)
- Parameters materialise into typed constants of story 02's runtime value
  shape: scalars parsed per the input's resolved type — YAML `1` into an
  integer type with range checked, not silently into something else — with
  one deterministic, documented rule for a literal that fits several accepted
  types; custom types through their registered ser/de hooks. A value that
  fits no accepted type is a compile error naming node, port, and expected
  type. (REQ-28, REQ-29, REQ-34)
- A parameter stored on a connected input is not an error: the connection
  feeds the node while it exists, and the stored value remains the fallback
  that returns when the input is unhooked — the same replace-and-restore the
  editor's connect and unhook flows assume. The precedence is written down
  where a hand-writer reads. (REQ-57, REQ-58)
- The proof, in the pattern of stories 01–02: the example binary compiles a
  definition shaped like the eventual fizzbuzz graph and prints the plan
  readable by eye — per node, ports with resolved types, inputs with their
  source or constant, edges with any conversion — then compiles a deliberately
  broken definition and prints each reported error, so error quality is
  verifiable, not just error existence.
- Focused tests cover the negotiation cases (exact overlap, single bridging
  conversion, ambiguous bridge, no bridge), double-fed inputs, cycle
  detection with the path named, parameter materialisation including a custom
  type via its ser/de hook, all-errors-in-one-pass, and the compiled graph's
  immutability; the workspace stays clippy-clean and `DEVELOPMENT.md` covers
  the extended example.

## Comments

- 2026-09-16: Boundaries. Story 03 delivers the definition model this
  consumes and deliberately leaves every semantic check to this story. Story
  02 hands over the runtime value and the conversion declarations applied
  here; materialising literals was explicitly left to this story. The
  conversion adapters are recorded, not run: values flow in story 06. How a
  node's logic is invoked is story 05's API — the compiled plan carries the
  resolved descriptor per instance so wiring needs no registry lookups.
  Mid-run error behaviour is story 06; this story owns only pre-run errors.
  How the error list reaches the editor is story 18; this story guarantees it
  is structured and actionable.

- 2026-09-16: The negotiation rule is written so story 09's generic numeric
  ports (a Counter over every numeric type) can participate without rework:
  whatever mechanism expresses them lands as port-side type sets, which is
  exactly what this rule consumes. The rare literal that fits several
  accepted types follows one rule — narrowest same-kind fit — which story 09
  may revisit for generic ports.

- 2026-09-16: Groups need nothing reserved. A group node compiles like any
  node — a node type backed by a subgraph, reachable through the same
  descriptor; whether the compiler inlines subgraphs or the engine nests
  execution is story 23's call, and nothing in the compiled graph's shape
  forecloses either.

- 2026-09-16: Deliberately left alone: which node a run "starts" from is not
  a compile concept — a streaming DAG drains from producers, and that is the
  engine's business (story 06). And the file gains no new syntax: story 03's
  optional edge annotation stays unused, since conversions are derived, not
  declared.
