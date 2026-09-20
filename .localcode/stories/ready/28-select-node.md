---
title: Select, an aligned chooser node
date: 2026-09-20
---

## Description

The fizzbuzz graph's next step (story 29) needs to pick between candidate
strings per count: "Fizz" against the formatted number, "FizzBuzz" against
"Fizz". Today that selection lives in a bespoke node — the fizzbuzz crate's
`Case selection`, whose behaviour hardcodes the three strings and the
pairing (crates/nodetool-fizzbuzz/src/lib.rs:237-292). Story 29 moves the
strings onto the graph; the pairing that makes it correct is worth keeping as
a node of its own, in the utility crate beside `If` and `Format` — and story
29's rewiring deletes `fizzbuzz/case` — `CaseSelection`, `case_text`,
`case_check`, `flag_input` and their tests with it. Select replaces the
bespoke node; nothing retained beside it.

Add a **Select** node: a boolean `choose` input, two value inputs `then` and
`else` of one type, one output `value` of that type. Per aligned triple — a
`choose` value with one value from each candidate — it emits the matching
candidate's value: `then` when the flag holds, `else` when it does not.

Its semantics are the alignment the `Case selection` behaviour already
proves (lib.rs:230-236, the buffering and lockstep pairing): each connected
input's arrivals are buffered, and the node emits only when all three hold a
value, consuming one from each front per emission. Reading the candidates'
current values instead would pair a choice against whichever neighbour
arrived last — the race the `Case selection` doc comment warns of. One
candidate may be a parameter literal (a constant that is always there) and
the other a connection; every parameter-held input — a literal candidate, a
parameter-held `choose` — is read as its current value when the pairing
fires, never buffered.

Declared, for now, over `String` — the type the fizzbuzz graph selects
between — through the ordinary `node_type!` path in `nodetool-utility`, with
the scalar helper-input rule carrying literal candidates (REQ-38). One
wiring constraint rides the placement: the fizzbuzz binary links only its
own plugin (crates/nodetool-fizzbuzz/src/main.rs:42), so story 29's rewiring
links the utility plugin — one dependency, one `use` — for `utility/select`
to compile there and for the fizzbuzz editor's palette to offer the node.

## Definition of done

- The utility plugin declares a Select node, visible in the palette of an
  editor linked to it — `cargo run -p visual` until story 29 links the
  utility crate into the fizzbuzz binary — under the utility header beside
  `If` and `Format`, wired and runnable like any node (REQ-8, REQ-9).
- A Select whose candidates mix a literal and a connection pairs each
  `choose` value with the connection's next buffered value, emitting once
  both have arrived, in arrival order; with both candidates connected, each
  emitted value pairs one-to-one with a choice in arrival order, never
  against a stale or future neighbour (REQ-16, REQ-17, REQ-21).
- Unfed inputs are rejected the way the fizzbuzz crate's gating nodes reject
  them — the compile-time carried-input check, `carried_check`
  (crates/nodetool-fizzbuzz/src/lib.rs:100-108) hoisted into `nodetool`
  beside `ParameterCheck` so both plugins' `check_parameters` share the one
  helper; Select is the utility crate's first declaration of it. No second
  copy, no new validation invented (REQ-72), and the fault surfaces at
  compile, before any run, not as a hung one (REQ-60).
- The plugin's tests cover: literal-and-connection candidates, two connected
  candidates arriving out of step, and a `choose` stream that ends while
  candidates keep arriving — the buffered choices still pairing with the
  later candidates in order, the node completing once every input has ended,
  emitting nothing for the candidates left unpaired (REQ-19, REQ-22,
  REQ-23) — and a `choose` held as a parameter literal, each candidate pair
  emitting once under the held choice, in arrival order (REQ-18).
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and
  the UI test suite pass.

## Comments

- 2026-09-20 — Declared over `String` only, deliberately: the generic-port
  family machinery lives in the fizzbuzz crate (`numeric.rs`), and no story
  needs a numeric or polymorphic Select yet. When one does, the port family
  is the way to widen it — one declaration, stamped per member — not a new
  descriptor concept. Until then a second type would be machinery with no
  consumer (REQ-2, REQ-72).
- 2026-09-20 — The behaviour body is the `Case selection`'s loop with the
  strings taken out: three `VecDeque`s, emit when all three fronts hold,
  pop in lockstep (lib.rs:253-263). The lockstep consumption is what keeps
  the chained selects of story 29 correct — each upstream emits exactly one
  value per count, so each consumer's fronts stay aligned without any
  timestamping or join machinery in the engine.
- 2026-09-20 — The literal candidate needs no arrival: an input held by
  parameter is read as its current value when the pairing fires, the same
  held-value rule `flag_input` uses (lib.rs:297). The behaviour reads every
  input as buffered-if-connected, current-if-literal, `choose` included: a
  parameter-held `choose` passes the carried check, but buffered it pairs
  its single arrival once and then falls silent — the run still completes
  at input-end, the later candidates dropped unpaired — read as current, it
  is always there, like a literal candidate. The two faces are REQ-18's —
  "a new value arrived" against "here's the current value" — and one
  declared rule carries both.
