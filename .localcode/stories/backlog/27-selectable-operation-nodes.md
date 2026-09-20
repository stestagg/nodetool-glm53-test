---
title: One binop node and one comparison node with selectable operations
date: 2026-09-20
---

## Description

The fizzbuzz plugin declares one node type per operation: six comparison
conditions (`fizzbuzz/eq` … `fizzbuzz/ge`, crates/nodetool-fizzbuzz/src/lib.rs:169)
beside the divisibility test (`fizzbuzz/divisible`, lib.rs:218). Seven palette
entries, seven types to learn — the six comparisons stamped from one behaviour
carrying a frozen `fn(T, T)`, the divisibility test its own `a`-gated body —
and the moment anyone wants a different operation, the plugin grows another
declaration.

Replace them with two node types whose operation is a per-instance setting:

- a **binop** node: two numeric inputs, one numeric output; operations at
  least add, subtract, multiply, divide, modulo.
- a **comparison** node: two numeric inputs, one bool output; operations
  equal, not equal, less, less or equal, greater, greater or equal.

The operation is selected on the instance, in the editor, and the node keeps
the generic-port idiom the crate already runs: one port family, resolved to a
concrete type at compile time, logic written once and stamped per type.

This needs one small generic descriptor concept in core: a node type can
declare a named **choice** — a parameter whose value is one of a fixed set of
options (the binop's `op`, the comparison's `cmp`). The instance stores the
choice among its parameters; the editor renders declared choices as a select,
not a free-text field; the compiler rejects a value outside the set, or an
instance left with none; the behaviour factory reads the choice from the
compiled node, compile having guaranteed it. The operation is a fixed
per-instance setting, not a connectable input — no port carries it; the
request's "with a port etc.." is the parameter idiom it rides, not a wire. A
choice's name is not an input port, so the compiler's parameter pass — which
today refuses any parameter that does not name one (compile.rs:292) — accepts
a declared choice's name as a parameter key beside the port-named parameters
it already carries: never connectable, never in the hang gate. The concept is
generic — it knows nothing about arithmetic (REQ-73) — and now has two
consumers, where the old comment in lib.rs:146 saw one and deferred.

The shipped `graphs/fizzbuzz.yml` is rewired onto the new nodes: each
`fizzbuzz/divisible` instance becomes a binop (modulo, `b` literal) feeding a
comparison (equal, `b` literal zero). The ticker sample
(`examples/visual/graphs/ticker.yml`), which instantiates `fizzbuzz/eq` and
`fizzbuzz/gt`, is rewired onto the comparison node as well — equal, `b` 7;
greater, `b` 100. The fizzbuzz graph's headless run prints the same hundred
lines as today.

## Definition of done

- A node type can declare a choice setting — name and fixed options — and
  instances hold their choice as an ordinary parameter value; nothing in the
  concept or its naming mentions arithmetic or any particular operation
  (REQ-73).
- The editor renders a declared choice as a select of its options — on the
  node's body and in the sidebar alike, the two views every value keeps —
  with the accessible naming the parameter fields carry, and the chosen
  operation is visible on the node in the canvas, so a graph of binops still
  reads without opening each one (REQ-37).
- An unset or out-of-set choice fails compile with a line naming the node
  and the choice it lacks — a fresh node's state is a compile error, not a
  run-start panic — and the problems recomputed beside every edit surface it
  before any run (REQ-60).
- The palette offers one binop node and one comparison node in place of the
  seven condition-family types; the comparison carries REQ-63, the binop is
  the original request's ask and no requirement names it; both run over the
  whole numeric family (REQ-27, REQ-8).
- The shipped graphs that instantiated a removed type are rewired —
  `graphs/fizzbuzz.yml`, and the ticker sample at
  `examples/visual/graphs/ticker.yml` — and the fizzbuzz graph's headless
  run prints the same hundred lines as before the change (REQ-68, REQ-75).
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and the
  UI test suite pass; the plugin's tests cover both new nodes across the
  family, including divide and modulo by a zero divisor on an integer
  member, ending the run as a reported failure (REQ-71).

## Comments

- 2026-09-20 — REQ-63 asks nodetool-fizzbuzz to define "a Condition operator
  node that ... applies a specified boolean operation". One comparison node
  whose operation is specified per instance satisfies this more directly than
  the seven declarations it replaces; no requirement names the individual
  condition types. The `Divisible` node's special stream shape (one boolean
  per `a` arrival) is not lost: with `b` held as a literal — as both shipped
  instances do — the generic pairing emits exactly one boolean per count, and
  the shipped graph is the proof. A streamed divisor now re-emits per arrival,
  like any binop; that is the generic rule, not a defect to patch around
  (REQ-73).
- 2026-09-20 — Where things live: the choice descriptor belongs beside
  `check_parameters` in `nodetool::node_type` (crates/nodetool/src/node_type.rs:33);
  the two node types belong in the fizzbuzz crate, where the numeric family
  helpers (`numeric.rs`) and the shared-behaviour stamping (`conditions!`,
  lib.rs:148) already are. The behaviour bodies barely change: the macro
  passes `$compare` per declaration today; the factory reads
  `compiled.parameters["op"]` and picks the same `fn(T, T) -> bool` /
  arithmetic fn from a table — a read compile has guaranteed: an unset or
  out-of-set choice never reaches it.
- 2026-09-20 — The choice rides the instance's existing parameter map as a
  string value; no new storage, no new protocol message. `set_parameter`
  stays unjudging, as for every parameter: the listing the UI already
  receives carries the options, the select offers only those, and the
  problems recomputed beside the edit name a stray value before any run. The
  compiler's parameter pass accepts a parameter naming a declared choice —
  today it refuses any parameter that does not name an input port — rejecting
  an out-of-set value against the descriptor's options: core's check, not a
  plugin `check_parameters` like the zero step's
  (crates/nodetool-fizzbuzz/src/lib.rs:110). One generic check over every
  declared choice, in core, so no node's `check_parameters` carries it.
- 2026-09-20 — The choice deliberately lives off-port — an operation is not a
  value that flows — so REQ-57's connect-by-default is not read to cover it,
  and the graph format's `parameters` doc line
  (crates/nodetool/src/graph.rs:169, "values fixed for input ports as
  literals, by port name") is reworded in the same change to own choice names
  beside the port-named parameters.
- 2026-09-20 — The condition sub-group in the palette shrinks to the two new
  types; whether the sub-group keeps the name `condition` or becomes something
  plainer is the implementer's call — the palette reads as an index either
  way. The node labels deserve the same plainness: "Binop" is the code word,
  and the palette is the index users scan, so a label from the user's
  vocabulary — "Binary operation" or "Math", "Compare" — fits better. The
  implementer's call, as for the sub-group.
