---
title: Fizzbuzz data logic on the graph, output keyed to the Output node
date: 2026-09-20
---

## Description

The fizzbuzz crate still knows it is a fizzbuzz app. The `Case selection`
node hardcodes the strings "Fizz", "Buzz", "FizzBuzz" and the selection
itself (`case_text`, crates/nodetool-fizzbuzz/src/lib.rs:285-292) — data and
decision that belong on the graph, where a user can read and change them.
And the binary's terminal rule is keyed to the wrong thing: every output the
graph leaves *unconnected* prints to stdout (crates/nodetool-fizzbuzz/src/main.rs:120-125),
so a graph with no Output node still prints, and output has nothing to do
with the Output node the user hooked up.

Two changes, one outcome — the Rust is a graph runner and a counter, and the
app is the graph file:

1. **The graph carries the logic.** `crates/nodetool-fizzbuzz/graphs/fizzbuzz.yml`
   is rebuilt from story 27's and 28's generic nodes: the counter's count
   streams into two binop/comparison pairs (modulo 3, equal 0 / modulo 5,
   equal 0) and a `Format` (the plain number); three `Select` nodes keyed to
   the two comparison flags pick "Fizz"/"Buzz"/"FizzBuzz", the constants
   held as parameter literals on the Select's candidate inputs. The `Case
   selection` node type and its hardcoded strings are deleted from the
   plugin; `Counter` and `Output` are the fizzbuzz-specific nodes that
   remain. The shipped graph gains an Output node, and the headless run
   prints the same hundred lines.

2. **Output is the run's product.** The binary prints values delivered to
   `fizzbuzz/output` nodes' inputs, as they arrive — headless and in UI mode
   alike. A graph with no Output node hooked up produces no stdout at all:
   a silent, correct run. Printing stays the host's rule; core learns
   nothing about output nodes (REQ-73).

## Definition of done

- The fizzbuzz plugin declares no node carrying fizzbuzz strings or the
  selection: the `fizzbuzz/case` type, `case_text`, and their tests are gone;
  the plugin's node Rust knows numbers, strings, and streams, not the app —
  the binary's printing rule, naming its own output type, is the host's, as
  the REQ-73 line below says (REQ-61, REQ-70).
- The binary's `plain` builds on `nodetool-utility`'s `string_form` — public
  in `nodetool-utility`, which the rewiring links anyway — and keeps only
  its non-scalar fallback: the rewiring links the utility plugin
  (story 28's assignment), so the hand-synced scalar twin and its now-false
  "linking that crate here" comment go (REQ-70).
- The shipped graph expresses the selection with generic nodes and parameter
  literals; every constant it needs to be fizzbuzz is visible in the file,
  editable in the editor (REQ-11, REQ-75), and the rebuilt nodes keep
  instance labels naming their role — "mod 3", "is zero", "pick FizzBuzz" —
  so the canvas and the file read without opening each one.
- Headless: a graph whose Output node is hooked up prints each delivered
  value on stdout as it arrives, one line each, in its plain string form; a
  reader that goes away still ends the run quietly, and a graph with no
  Output node prints nothing and still runs to completion (REQ-68). A
  literal held on the Output node's input is a value it receives too, and
  prints once.
- A run started from the browser over `--ui` prints to the terminal exactly
  as the headless run does — the rule follows the graph the run compiles, so
  an Output node added and wired in the browser prints too, and one deleted
  there stops (REQ-69).
- The Output node still collects its input into a vec, unchanged, for
  programs that want the run's product (REQ-66).
- The machinery the change orphans goes with the rule it carried:
  `engine::unconnected_outputs` — this binary and the server's spawn being
  its only two callers — is deleted, and core's `server_host.rs` no longer
  exercises a hook the product no longer supplies: rewritten to the hook the
  choice leaves, or dropped with the consumer — the file's unsaved-changes
  guard test is not the hook's: story 24's process-boundary settlement, so it
  keeps a home whatever the hook becomes, moved with its concern and never
  dropped with the file; `Run::consume` stays as the generic mechanism a
  future host rebuilds on (REQ-73).
- Nothing in core or the server names or branches on the output node type;
  the printing rule is the binary's, expressed through generic mechanisms
  (REQ-73).
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and the
  UI test suite pass; an end-to-end test runs the shipped graph headless and
  checks the hundred lines.

## Comments

- 2026-09-20 — REQ-68 says headless "just prints the output values as they
  arrive" — unchanged; what changes is which values those are: the ones an
  Output node receives, not every unconnected output. The unconnected-output
  printing rule has no other host to stay for: `run-graph` consumes named
  outputs it chooses per sample (examples/run-graph/src/main.rs:38) and
  `visual` supplies no printer at all (examples/visual/src/main.rs:47) — the
  rule lives only in this binary, and this story replaces it, the machinery
  beneath it going with the change.
- 2026-09-20 — Mechanism, both doors, one rule. The clean generic move is an
  input-side twin of `Emitted` — a `Delivered { node, port, value }` event,
  the value as the input receives it, past the connection's conversion —
  the twin of `Emitted`'s "before any conversion"
  (crates/nodetool/src/engine.rs:567-569) — which the binary filters by its
  own plugin's output type, named on the events, and prints. The
  alternative is widening the consumer mechanism — a
  host-named tap on an input instead of an unconnected output, the host
  naming its interest by its own plugin's types, the server resolving the
  instances against the compiled graph at spawn. One property the "touches
  less" rule misses: a consumer keeps the bounded hand-off's backpressure —
  a stalled terminal stalls the graph, memory bounded — while an observer's
  queue is unbounded by design, a stalled terminal buffering without bound.
  The weigh resolves to the consumer side. Output is the run's product, and
  the product belongs on the product channel — `Run::consume`'s bounded
  hand-off, whose tasks the run joins (engine.rs:140-146), so the binary
  awaits `start` plainly and no tail is lost — not on the observer's
  unbounded witness queue, whose tail would need the RunFinished note to
  survive the current-thread exit. The engine gains nothing new: no
  `Delivered` variant, no `type_ref` on the event identity. The host names its taps by its own plugin's output type;
  the server resolves them against the compiled graph at spawn, as the hook
  widening below already says. Do not add a sink flag to node descriptors
  for this — the host already knows its own plugin's types. A printer cannot
  fail into the engine (engine.rs:609-614), so the binary keeps the stop
  channel it already holds and its printer sends on it when a write fails —
  a reader gone still ends the run quietly, the consumer holding the stop
  channel today.
- 2026-09-20 — The UI door's printer currently rides the server's
  `consume_unconnected` callback (main.rs:155), the one hook a host has into
  the server's runs; the server builds the run itself and subscribes its
  bridge as the one observer (crates/nodetool/src/server/run.rs:84-96) — the
  canvas's statuses, pulses, and port values all ride it (REQ-14) — and
  `Editor::subscribe` is the browser's wire, not the run's. The rule follows
  the graph the run compiles: over `--ui` the user edits the session's
  definition between runs, and the binary's launch-time copy — stale, or
  empty when launched with no file — knows nothing of an Output node added
  there, so the host's interest is named by its own plugin's types, never
  resolved from its own definition: the host names its taps by type and the
  server resolves them against the compiled graph at spawn. The mechanism
  widens that hook, generically: the hook carries the taps the host names.
  The unconnected consumer is not left unsupplied but removed, with
  `engine::unconnected_outputs`, which only it and this binary called — its
  hook surviving widened, the unconnected form gone with the rule — and
  core's `server_host.rs` rewritten to the replacing hook or dropped with
  it; `Run::consume` stays as the generic mechanism a future host rebuilds
  on. The editor's own run surfaces are untouched.
- 2026-09-20 — The unfed Output trap, for the record: with printing keyed to
  the Output node, the likeliest accidental silence is an Output node
  dropped but never wired — its single `text` input escapes the hang gate,
  which skips a node whose every input starves
  (crates/nodetool/src/compile.rs:354-356) — so Start hangs with no warning,
  no mark, and no stdout. That is today's settled behaviour and this story
  keeps Output unchanged: deliberately left, recorded so the silence is
  owned; a later story may widen the gate.
- 2026-09-20 — Graph shape, for the record: `count → (binop mod 3, cmp eq 0)`
  and `count → (binop mod 5, cmp eq 0)` give the two flags; `count → Format`
  (with `template: "{}"` — the plain form through the single substitution,
  and typeable where the empty-string spelling is not: the editor unsets on
  an empty commit and cannot store an empty string,
  crates/nodetool/ui/src/fields.jsx:61-63; left unset the node never fires
  and the run hangs, crates/nodetool-utility/src/lib.rs:73-76) gives the
  plain string; `select(buzz, "FizzBuzz", "Fizz")`,
  `select(buzz, "Buzz", formatted)`, and `select(fizz, inner1, inner2)` give
  the text into the Output node. The Selects' lockstep pairing is what makes
  the parallel branches safe — that is story 28's semantics, not new engine
  work.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
