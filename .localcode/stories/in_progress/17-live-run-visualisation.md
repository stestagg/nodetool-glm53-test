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
and stopped when a fail-fast or user-stopped run abandons it — the derived
status story 07 defines, rendered; there is no second status mechanism to
render from. The last run's statuses persist after the run ends,
until the next start resets them, so the node a failure happened in stays
findable after the text of the error has scrolled away. **Values**: each
emitted value animates the wire or wires it travels — a fan-out pulses every
downstream wire, each one a path the value actually takes — and shows as
small text at the emitting output port, replaced by each new emission. The
values persist like the statuses, until the next start resets them — the
counter's last ticked value is evidence of what the run did — and ride the
connect-time snapshot beside them, so a browser connecting after the run
sees the same canvas the first tab does. Values of core base scalars display
as their scalar text; values of plugin custom types animate the flow but
carry no invented content — their rendering is plugin territory (story 21),
and core stays opaque to them (REQ-33). The canvas answers "what is
happening where" at a glance; a scrolling log is what the console is for.

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
attach to the live stream. Yes — and not only mid-run: a browser connecting
at any time is given the current state. The connect-time resync already
carries run state (16); it now also carries a snapshot of the current
per-node derived status and the latest value held per emitting port — the
statuses and values the bridge holds, mid-run's or the last run's, for while
they persist they are current state — and events from that moment flow to
every connection like any server push. No event history is replayed — events
are happenings, not state, and the snapshot is the state. A second tab, a
reload, or a reconnect shows a canvas as close to the first tab's as the
moment allows, keeping the tabs-agree rule every story since 12 has held,
and giving story 18 a settled path for the reconnect-after-connection-loss
case it will surface.

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
  source beside the events. Editing stays locked per 16; the animation is
  view-only. (REQ-14, REQ-13, REQ-45)
- Node status is derived from the events alone, once, in the bridge — the
  single subscriber — keyed by instance uuid, held there, and pushed as state
  beside 16's run state: a node shows running from its started event,
  completed when it completes, failed when it errors, and stopped when a
  failed or user-stopped run-finished closes a started node without its own
  final transition — story 07's closure rule, extended to the stopped
  outcome 16 added, with no per-node "aborted" event invented to render. The
  canvas renders the pushed statuses and derives none of its own — one
  transition table, not two. The last run's statuses persist after the run
  ends — completed, failed, or stopped alike — until the next start resets
  the canvas as its events arrive, so a failure stays locatable after the
  run. (REQ-43, REQ-14, REQ-41)
- Each emitted value animates the wire or wires it travels — a fan-out
  pulses every downstream wire — and displays as small text at the emitting
  output port, replaced by each new emission. Values of core base scalars
  display as their scalar text; values of plugin custom types animate the
  flow but display no invented content, their rendering being plugin
  territory (21). Like the statuses, the last run's values persist after the
  run ends — the counter's last ticked value is evidence of what the run
  did — until the next start resets the canvas, and they travel the
  connect-time snapshot beside the statuses, so a tab connecting later sees
  the same canvas. The node stays compact: a small per-port readout, never a
  log. (REQ-14, REQ-29, REQ-33, REQ-47, REQ-73)
- The transport forwards every event, values included — no sampling,
  thinning, or aggregation server-side; one event model crosses the bridge
  untouched. The cheapness lives in rendering: the browser coalesces, capping
  the pulse rate and holding only the latest value per port, so a high-rate
  graph animates smoothly and may drop animation frames but the statuses and
  latest values it shows are always the events' own. The run never waits on
  any browser: a slow, stalled, or dead connection affects neither the run's
  values, timing, nor completion. The bridge's per-connection queue is
  bounded, and a connection that cannot keep up is dropped — a loss 18
  surfaces — rather than the transport thinning or the run waiting. (REQ-2,
  REQ-13)
- A browser connecting at any time — first open, reload, second tab, or a
  reconnect — is pushed the run state (16) plus a snapshot of the current
  per-node derived status and the latest value held per emitting port — the
  current displayed state, mid-run's or the persisted last run's — and
  receives events from then on; no event history is replayed. Every open tab
  animates the same live stream and agrees with the others as far as the
  moment allows. (REQ-14, REQ-2, REQ-45)
- Focused tests cover the bridge server-side: run events forwarded to every
  connected socket as they occur; a connection's connect-time resync carrying
  the run state and the per-node snapshot — the statuses and latest values
  the bridge holds, empty only before the session's first run, the last
  run's riding the snapshot for as long as they persist; scalar emissions
  carrying the value in its browser-renderable form, and custom-typed
  emissions crossing with no value content — the event forwarded, the wire
  animated, nothing core would have to invent; statuses deriving as story 07
  defines, extended to 16's stopped outcome — including started-but-closed
  nodes deriving stopped on a failed run-finished and on a user-stopped one;
  a stalled and a closed connection during a run leaving the run's values,
  timing, and completion untouched, and a connection dropped once its
  outbound queue passes its bound, the run likewise untouched; pushes fanning
  out to several connections. The workspace builds and passes `cargo test`
  and `cargo clippy` cleanly, with the UI build part of the check set
  DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: launch the editor example and
  open the small streaming sample it ships — the editor example links the
  fizzbuzz plugin crate, as 10's binary links it, so the Counter is in its
  palette and the shipped sample streams; this story's work extends the
  sample with a source emitting a rising count and a node that fails when a
  parameter asks, and it runs long enough to watch — and press Start: nodes
  mark running as they start and completed as they finish, pulses travel the
  wires, and the value at the output port ticks as the counter counts; open
  a second tab mid-run and see the same statuses and values, joining the
  stream live; make one node fail deliberately and watch it mark failed
  while the nodes it would have fed mark stopped, both marks still there
  after idle returns — and still there, with the last values beside them,
  after a reload; press Stop on a later run and see stopped marks; start
  again and watch the canvas reset and animate afresh. DEVELOPMENT.md
  describes how to run it.

## Comments

- 2026-09-16 — Scope seams: this story's additions to story 11's message
  catalogue are the event pushes and the status-and-value snapshot riding the
  connect-time resync; the start/stop commands and run state are 16's, and
  the question 16 deferred — whether a browser connecting mid-run attaches to
  the live stream — is settled here: yes, at any time. Systematic error
  surfaces and connection-loss surfacing are 18's — the failed-node mark here
  locates a failure, and a connection dropped past its bounded queue is a
  loss this story's bridge causes; 18 explains the failure and surfaces the
  loss. Palette icons, colours, and grouping are 19's; multi-selection 20's;
  custom-type value rendering 21's; the a11y pass 22's; the fizzbuzz UI mode
  consuming all of it 24's. REQ-14 completes across 16 and 17: there the
  editor's runs are observed and the chrome driven by the events, here the
  nodes animate. (REQ-74)
- 2026-09-16 — UX calls settled here: latest value per output port rather
  than a value log — a log is the console's job (the headless printing
  observer, and 24's console), while the canvas answers what-is-happening-
  where at a glance; statuses persisting until the next start rather than
  clearing when a run ends, so a failure stays locatable after the outcome
  text is gone; the values persisting alongside them for the same reason —
  the counter's last ticked value is evidence of what the run did — and
  riding the connect-time snapshot, since persisted display is current state
  and a tab connecting later must agree; fan-out pulsing each wire, because
  each wire is a path the value actually travels; and no animation toggle or
  settings — nothing to configure, one experience, honouring the platform's
  reduced-motion preference, which is not a setting the app offers. The
  status marks are distinct states on the node, not the type colouring 19
  brings; their contrast meets the floor 22 passes over.
- 2026-09-16 — Why every value crosses and no transport thinning: the editor
  must not be less truthful than the headless terminal, whose observer prints
  every value (07); sampling would hide exactly the bursts a user watches
  for; and the loopback single-user scope means volume is not a problem to
  solve. Where the cost really is — browser rendering — the answer is
  coalescing: animation frames may drop, the derived truth never. A sampled
  event path beside the real one would be exactly the second mechanism the
  vision forbids. (REQ-2)
- 2026-09-16 — Why the bridge owns the one status derivation: the
  connect-time snapshot forces the derived-status table to exist server-side
  — the bridge must watch the events to answer a connect at any time — and
  the events cross untouched anyway, the values needing them. Letting the
  canvas derive the same table again from the lifecycle events it receives
  would put the transition table in Rust and in JS, and drift between the
  two would surface as tabs disagreeing — the second status mechanism the
  story forbids arriving by the back door. One derivation, in the bridge,
  pushed as state beside 16's run state; the canvas renders and derives none
  of its own. "From the events alone" survives — the events are the only
  input — and 18 inherits one derivation, not a reconciliation between two.
- 2026-09-16 — Why attach means snapshot-plus-live-events, not history: the
  tool's standing rule is that the browser is a synced view of server state;
  events are happenings, not state — what a new connection can be given is
  the current state, then the happenings from now on. This extends 16's line
  — the resync carries the state, not a run's event history — from run state
  to per-node status and the latest per-port values, mid-run's or the
  persisted last run's, and gives 18's reconnect-after-loss the same path as
  any first connect, with nothing extra to build.

- 2026-09-18: Implementation got stuck at "implement story": opencode run failed (143). The story went back to ready to be picked up again.
