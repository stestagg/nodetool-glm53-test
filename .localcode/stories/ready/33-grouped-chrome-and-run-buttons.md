---
title: Grouped chrome with start, stop, and reset
date: 2026-09-20
---

## Description

The header's five buttons sit in one undifferentiated row
(crates/nodetool/ui/src/editor.jsx:795-801): the run toggle lives between the
file name and New/Open/Save, so the two kinds of action — tending the
document, driving the run — read as one list. And the run control is one
Start/Stop toggle (view.js:261-268), which leaves no seat for the reset a
user wants after watching a run: back to the idle canvas without saving or
reopening anything.

Regroup the header into two visually distinct groups, and give the run group
its three controls:

- **Graph/file group**: New, Open, Save, Save as — everything that acts on
  the document.
- **Run group**: Start, Stop, Reset — everything that acts on the run.
  (Step and friends land here later; the group leaves them room.)

Start begins a run (as the toggle's Start does); Stop requests the running
graph's stop, enabled only while a run is on; Reset returns the editor to
the idle state — a running run is stopped first, then statuses, port values,
and the finished outcome clear, ready for a fresh Start. Reset is a small
server operation beside `start_run`/`stop_run`: end the run if one is on,
discard the run state, report the idle state back (REQ-26's recompile-on-
restart is untouched — Start after Reset compiles as Start after Stop does).

## Definition of done

- The header shows the file actions and the run actions as two visibly
  separate groups — grouping by spacing or an outline, consistent with the
  Blueprint look (REQ-50, REQ-49).
- Start, Stop, and Reset are separate controls: Start enabled when idle,
  connected, and the graph has nodes; Stop enabled only while running;
  Reset enabled whenever a run holds any state (running or finished)
  (REQ-59; the reset control is the original request's ask, and no
  requirement names it).
- Reset while running stops the run, then clears run state — statuses, port
  value readouts, and outcome — so the canvas reads idle and stays idle:
  the run's own finish push cannot bring the stopped outcome or the
  cleared statuses back, and no new Start is needed; a Start after Reset
  recompiles as today (REQ-26).
- The editor never sends a reset with no run state — the control is
  disabled without one — so the server's named refusal, as `stop_run`'s
  "no run is on to stop", is the backstop for a stale tab or a race, not a
  path a user hits: a named error, and nothing else changes — the editor,
  already idle, stays as it is (REQ-71).
- The run controls stay disabled while disconnected; the button-state tests
  in view.test.js cover the three controls across idle, running, finished,
  and disconnected (REQ-59).
- The UI test suite, `cargo test --workspace`, and `cargo clippy --workspace
  --all-targets` pass.

## Comments

- 2026-09-20 — The server's run state is already a small enum-shaped thing
  (`session.run`, crates/nodetool/src/server/mod.rs:655-706); `reset_run` is
  stop-if-running plus a transition to the empty state, and the existing
  run-state message carries the new idle state to every connection — no new
  event kind. But the run's ending is the bridge's, not the operation's:
  the stop is only a signal, and the engine's `run_finished` arrives later,
  where the bridge's run-finished handler unconditionally writes the
  outcome back into `session.run` and pushes it (bridge.rs:246),
  re-marking the nodes it still reads as Running stopped on the way
  (bridge.rs:233-244). A stop request is not a stopped run: a naive
  transition lets "run stopped" and the cleared statuses come back after
  Reset, and any emission or node-status event still in flight between the
  stop and the ending repaints statuses and port values the reset cleared.
  This is the first operation that ends a run out-of-band relative to its
  event stream, and the run-display model has no notion of "this run's
  events no longer apply". Of the two shapes — waiting for the finished
  event, or guarding the bridge against a run already reset — the story
  settles on the guard: the run carries a generation, spawned with it and
  tagging its events, and the bridge drops every event whose generation is
  not the run that is on. A finished outcome for a run the reset already
  ended is swallowed at the ending instead of resurrecting it, the
  in-flight statuses and emissions die with the discarded run, and a Start
  after Reset cannot inherit the dead run's events. A wait cannot hold the
  session lock until the run ends — the finish event needs that same lock
  to be delivered — so waiting means releasing and re-acquiring it; the
  generation is also the same piece a later step or restart control will
  want. And the emptied display travels as the existing `run_display` push
  (the connect-time snapshot's kind, mod.rs:316): the run-state message
  alone does not clear what the tabs show — the wire clears statuses and
  values only when the new state is running (wire.js `onRun`) — so after
  `session.display.reset()` the reset pushes the emptied display, and
  every connection's canvas clears with the state.
- 2026-09-20 — The client's `runControl` (view.js:261-268) returns one
  {label, enabled} today; it becomes three {enabled} flags and the flip
  goes — the toggle's label logic and the wire's stop-or-start branch
  (wire.js:87) dissolve with it. The tests at view.test.js:327-361 name
  the shape to rewrite. Keep the state derivation pure and tested there;
  the header renders it.
- 2026-09-20 — Grouping is presentation, not structure: two containers in
  the `.chrome` header (editor.jsx:789-802) with the existing button styles;
  resist promoting this into a generic toolbar component — five buttons and
  two groups need none (REQ-2, REQ-70).

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
