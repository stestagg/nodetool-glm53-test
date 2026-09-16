---
title: Live run visualisation
date: 2026-09-16
---

## Description

Story 16 gave the user the run: a start/stop control, a locked-while-running
canvas, an outcome in the chrome. But the canvas itself is still furniture —
the graph the user built and wired sits frozen while it works, and the user
must read the chrome to know anything is happening at all. The engine has had
a voice since story 07 and the transport a push channel since 11; 16 already
drives its run state from the events. This story gives that voice to the
canvas: the event stream reaches the browser and the nodes animate as
execution proceeds — the vision's "press start and watch values flow" made
real in the visual experience (REQ-14).

While a run is on, two things show. **Node status**: a node visibly runs from
its started event, shows completed when it completes, failed when it errors,
and stopped when a fail-fast or user-stopped run abandons it — exactly the
derived status story 07 defines, rendered; there is no second status
mechanism to render from. The last run's statuses persist after the run ends,
until the next start resets them, so the node a failure happened in stays
findable after the text of the error has scrolled away. **Values**: each
emitted value animates the wire or wires it travels — a fan-out pulses every
downstream wire, each one a path the value actually takes — and shows as
small text at the emitting output port, replaced by each new emission. Values
of core base scalars display as their scalar text; values of plugin custom
types animate the flow but carry no invented content — their rendering is
plugin territory (story 21), and core stays opaque to them (REQ-33). The
canvas answers "what is happening where" at a glance; a scrolling log is what
the console is for.

One of the vision's open questions settles here: how much runtime data the
event stream carries to the UI. The answer is all of it — every event
forwarded, values included, with no sampling, thinning, or aggregation at the
transport. The editor should not be less truthful than the headless terminal,
whose printing observer shows every value; sampling would hide exactly the
burst the user is watching for; and the loopback, single-user posture makes
volume a non-problem to solve. The cheapness the question worries about lives
where the cost actually is — rendering: the browser coalesces, capping the
pulse rate and keeping only the latest value per port, so a fast graph
animates smoothly by dropping animation frames rather than queueing a
backlog. Animation may thin under load; the derived truth — the statuses and
the latest values — stays the events' own. And the run never waits on the
browser: story 07's guarantee that notifying never touches the run holds
through the bridge, so a slow, stalled, or dead connection affects neither
the run's values, timing, nor completion.

The other call 16 deferred to here: whether a browser connecting mid-run can
attach to the live stream. Yes. The connect-time resync already carries run
state (16); it now also carries a snapshot of the current per-node derived
status, and events from that moment flow to every connection like any server
push. No event history is replayed — events are happenings, not state, and
the snapshot is the state. A second tab, a reload, or a reconnect shows a
canvas as close to the first tab's as the moment allows, keeping the
tabs-agree rule every story since 12 has held, and giving story 18 a settled
path for the reconnect-after-connection-loss case it will surface.

Deliberately not here: systematic error surfaces — toasts, panels, and the
connection-loss banner (18), though the failed node marked here is the
location 18's explanations will point at; icons, colours, and shapes for
types (19) — the status marks this story needs are states, not decoration;
multi-selection (20); custom-type value rendering (21); the accessibility
pass (22); the fizzbuzz binary's UI mode, which consumes all of this and
prints outputs to the console (24). Editing stays locked while running
exactly as 16 settled — animation is view-only, and nothing here reopens that
lock.

## Definition of done

- While a run is on, the canvas animates from the engine's event stream: one
  bridge — story 07's observer composed as a single subscriber — forwards the
  events over the story 11/16 push path to every connection, and the canvas
  renders them; there is no second event model, no polling, and no status
  mechanism beside the events. Editing stays locked per 16; the animation is
  view-only. (REQ-14, REQ-13, REQ-45)
- Node status is visibly rendered from the events alone, keyed by instance
  uuid: a node shows running from its started event, completed when it
  completes, failed when it errors, and stopped when a failed run-finished or
  a user stop closes it without its own final transition — the derived status
  story 07 defines, with no per-node "aborted" event invented to render. The
  last run's statuses persist after the run ends — completed, failed, or
  stopped alike — until the next start resets the canvas as its events
  arrive, so a failure stays locatable after the run. (REQ-43, REQ-14,
  REQ-41)
- Each emitted value animates the wire or wires it travels — a fan-out
  pulses every downstream wire — and displays as small text at the emitting
  output port, replaced by each new emission. Values of core base scalars
  display as their scalar text; values of plugin custom types animate the
  flow but display no invented content, their rendering being plugin
  territory (21). The node stays compact: a small per-port readout, never a
  log. (REQ-14, REQ-29, REQ-33, REQ-47, REQ-73)
- The transport forwards every event, values included — no sampling,
  thinning, or aggregation server-side; one event model crosses the bridge
  untouched. The cheapness lives in rendering: the browser coalesces, capping
  the pulse rate and holding only the latest value per port, so a high-rate
  graph animates smoothly and may drop animation frames but the statuses and
  latest values it shows are always the events' own. The run never waits on
  any browser: a slow, stalled, or dead connection affects neither the run's
  values, timing, nor completion. (REQ-2, REQ-13)
- A browser connecting while a run is on — first open, reload, second tab, or
  a reconnect — is pushed the run state (16) plus a snapshot of the current
  per-node derived status, and receives events from then on; no event history
  is replayed. Every open tab animates the same live stream and agrees with
  the others as far as the moment allows. (REQ-14, REQ-2, REQ-45)
- Focused tests cover the bridge server-side: run events forwarded to every
  connected socket as they occur; a mid-run connection's connect-time resync
  carrying the run state and the per-node status snapshot, and carrying no
  statuses when idle; scalar emissions carrying the value in the
  browser-renderable form and custom-typed emissions carrying none; statuses
  deriving exactly as story 07 defines, including started-but-closed nodes
  deriving stopped on a failed run-finished; a stalled and a closed
  connection during a run leaving the run's values, timing, and completion
  untouched; pushes fanning out to several connections. The workspace builds
  and passes `cargo test` and `cargo clippy` cleanly, with the UI build part
  of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: launch the editor example and
  build the small streaming sample it ships — long enough to watch — and
  press Start: nodes mark running as they start, pulses travel the wires, and
  the value at the output port ticks as the counter counts; open a second tab
  mid-run and see the same statuses, joining the stream live; make one node
  fail deliberately and watch it mark failed while the nodes it would have
  fed mark stopped, both marks still there after idle returns; press Stop on
  a later run and see stopped marks; start again and watch the canvas reset
  and animate afresh. DEVELOPMENT.md describes how to run it.

## Comments

- 2026-09-16 — Scope seams: this story's additions to story 11's message
  catalogue are the event pushes and the per-node status snapshot riding the
  connect-time resync; the start/stop commands and run state are 16's, and
  the question 16 deferred — whether a browser connecting mid-run attaches to
  the live stream — is settled here: yes. Systematic error surfaces and
  connection-loss surfacing are 18's — the failed-node mark here locates a
  failure; 18 explains it and surfaces the loss. Palette icons, colours, and
  grouping are 19's; multi-selection 20's; custom-type value rendering 21's;
  the a11y pass 22's; the fizzbuzz UI mode consuming all of it 24's. REQ-14
  completes across 16 and 17: there the editor's runs are observed and the
  chrome driven by the events, here the nodes animate. (REQ-74)
- 2026-09-16 — UX calls settled here: latest value per output port rather
  than a value log — a log is the console's job (the headless printing
  observer, and 24's console), while the canvas answers what-is-happening-
  where at a glance; statuses persisting until the next start rather than
  clearing when a run ends, so a failure stays locatable after the outcome
  text is gone; fan-out pulsing each wire, because each wire is a path the
  value actually travels; and no animation toggle or settings — nothing to
  configure, one experience. The status marks are distinct states on the
  node, not the type colouring 19 brings; their contrast meets the floor 22
  passes over.
- 2026-09-16 — Why every value crosses and no transport thinning: the editor
  must not be less truthful than the headless terminal, whose observer prints
  every value (07); sampling would hide exactly the bursts a user watches
  for; and the loopback single-user scope means volume is not a problem to
  solve. Where the cost really is — browser rendering — the answer is
  coalescing: animation frames may drop, the derived truth never. A sampled
  event path beside the real one would be exactly the second mechanism the
  vision forbids. (REQ-2)
- 2026-09-16 — Why attach means snapshot-plus-live-events, not history: the
  tool's standing rule is that the browser is a synced view of server state;
  events are happenings, not state — what a new connection can be given is
  the current state, then the happenings from now on. This extends 16's line
  — the resync carries the state, not a run's event history — from run state
  to per-node status, and gives 18's reconnect-after-loss the same path as
  any first connect, with nothing extra to build.
