---
title: Interactive start and stop
date: 2026-09-16
---

## Description

Stories 12–15 gave the user an editor that builds: nodes dropped, arranged,
wired, valued, and kept in files — but the graph it builds has never run in
front of them. The engine runs headless (06, 10); in the editor the user
assembles a graph they can only look at. This story closes the editing loop
with the run: a start/stop control in the chrome, so the cycle the vision
calls first-class — build, run, stop, fix, go again — happens in one place.

The control is one button that flips: Start when idle, Stop while running,
sitting beside story 15's file controls in the chrome. There is no separate
"interactive mode" to switch on — the editor session is interactive mode, and
the control is simply there (REQ-59).

Pressing Start hands the definition the server holds to the compiler (04):
every start compiles afresh, with no compiled graph kept between runs, so
whatever the user edited last is exactly what runs — the recompile REQ-26
asks for is not a remembered optimisation but the only path. A clean compile
starts the run and the chrome turns over to running. A failing compile
reports the errors, naming what and where; the run never starts, the state
stays idle, and editing stays exactly as free as it was — the editor never
polices, compile decides (REQ-60), and a broken graph is something the user
fixes and retries, not something the UI forbids.

While a run is on, the graph it is running must not change (REQ-24): every
editing gesture goes quiet — palette drops, moves, wires and unhooks,
deletion, label and parameter edits, and file open, save, and fresh-graph.
The UI disables the controls (REQ-25), and the server backs the lock by
rejecting any edit operation that arrives while a run is on, the definition
untouched. Selection, panning, and zooming stay live — looking is not
editing — and the chrome's running state is the explanation the quiet
controls need; nothing is silently dead.

The run ends three ways, and the chrome tells them apart — and story 05's
semantics allow a fourth that is no end at all: a node with an input neither
connected nor parameterised (while another input is connected) never fires,
its upstream stalls, and the run hangs without error until Stop ends it — a
graph the shipped plugins make trivial to build. Stop is the exit for that
case too, so it must end a run that is making no progress. Every node
completing ends the run by itself: the state returns to idle, the outcome
shows completed, and editing re-enables without anyone pressing Stop (REQ-22).
A node erroring ends it fail-fast (06): the error is reported naming the node
instance and what failed, the outcome shows failed, same return to idle. And
pressing Stop ends it promptly: remaining work stops, delivery of values
still in flight is not promised — this narrows story 06's await guarantee
("awaiting the run suffices to have received every value") to hold for
completion and fail-fast, the user-triggered stop the one way a run ends with
undelivered values abandoned, adopting 06's own fail-fast cancellation
posture — and the outcome shows stopped. In every ending the compiled graph
and the definition are left untouched, so the next start begins clean and
fast.

Run state — idle or running, and the last run's outcome — is server state
like the current file (15's rule): pushed to every connection when it changes
and carried in the connect-time resync, so a second tab and a reload agree
with the first, and a start or stop made in one tab is visible in the others.
The resync carries the state, not a run's event history — whether a browser
connecting mid-run can attach to the live event stream is story 17's call.

Deliberately not here: nodes animating as values flow, and how much value
data the transport forwards (17); the systematic error surfaces — toasts,
panels, connection loss — that 18 builds (compile and run errors here are
plainly visible, and that is all); multi-selection (20); the fizzbuzz
binary's UI mode, which consumes this capability and prints outputs to the
console (24).

## Definition of done

- The editor chrome carries one start/stop control beside the file controls:
  Start when idle, Stop while running, with no separate interactive-mode
  toggle — the editor session is the interactive mode. With no nodes in the
  held definition the control is disabled: there is nothing to run, and a
  start arriving for an empty definition is answered with an error naming it,
  the state left idle. (REQ-59)
- Pressing Start compiles the definition the server holds — every start
  compiles afresh, no compiled graph survives a run, so an edit made since
  the last run is always what runs next — and on a clean compile the run
  begins: the chrome shows running, and every connection is pushed the new
  run state. The thing that runs is the story 04 compiled graph. (REQ-26,
  REQ-15, REQ-45)
- A compile failure reports the errors, naming what and where, visibly in
  the editor; the run never starts, the state stays idle, and editing remains
  fully enabled — enforcement is compile time's, and the editor never blocks
  editing on it. (REQ-71, REQ-60)
- While a run is on, editing is disabled: palette drops, node moves, wiring
  and unhooking, node deletion, label and parameter edits, and file open,
  save, and fresh-graph are inert in the UI, and any edit operation that
  arrives is rejected by the server with an error naming the running state,
  the definition untouched. Selection, panning, and zooming remain available.
  (REQ-24, REQ-25)
- Pressing Stop ends the run promptly — remaining work stops, in-flight
  delivery is not promised, and a run that is making no progress because a
  node's input never fires (story 05's gate) is still ended — the outcome
  shows stopped, distinct from completed and failed, and editing re-enables
  immediately. The engine gains this stop beside its natural ends
  (completion, fail-fast), not a second run model; a stopped run leaves the
  compiled graph and definition untouched, so the next start begins clean.
  (REQ-59, REQ-24)
- The run ends by itself when every node is complete — idle again, outcome
  completed, editing re-enabled without anyone pressing Stop; a run ended by
  a node's error reports the error naming the node instance and the failure,
  outcome failed, same return to idle. (REQ-22, REQ-71)
- Run state — idle or running, and the last run's outcome — is server state:
  pushed to every connection when it changes and carried in the connect-time
  resync, so a second tab and a reload mid-run show the same state, and a
  start or stop made in one tab is visible in the others; the resync carries
  the state, not a run's event history. (REQ-2, REQ-45)
- The state the chrome shows tracks the engine's own run lifecycle — run
  started and run finished from the story 07 event stream drive it, no second
  run-tracking mechanism — the same stream story 17 will bridge to the
  browser. The stopped outcome arrives as story 07's reserved extension of
  the run-finished event — "extendable when a later story adds another way a
  run ends" — so the engine's stop ends the run with a run-finished carrying
  that outcome, and the chrome's stopped state is derived from that same
  event stream, not a second signal beside it. (REQ-13, REQ-14)
- Focused tests cover: start compiling the held definition, with a run
  reflecting an edit made since the previous run; a compile failure answered
  with the errors and no run; each editing operation rejected while running
  with the definition untouched; stop ending a running run promptly with the
  stopped outcome, including a run stalled on an input that never fires;
  natural completion and fail-fast each returning to idle with their
  outcomes; start-while-running, stop-while-idle, and a start on an empty
  definition each answered with the error, the state left idle and the
  connection usable; run state pushed to every connection and present in the
  connect-time resync; a stopped run followed by a clean restart. The
  workspace builds and passes `cargo test` and `cargo clippy` cleanly, with
  the UI build part of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: drop and wire a small graph and
  press Start — it finishes by itself, idle returns, and editing works again
  with nobody pressing Stop. Start the shipped long-running sample (the
  example plugins gain a node whose behaviour takes a count and emits its
  values with a pause between them — without it every node the shipped
  plugins build completes in microseconds — and the sample runs it to a
  large count): running shows, and every editing gesture is off — a palette
  drag, a port drag, a sidebar field, and file save do nothing — while
  selection, panning, and zoom stay live; press Stop mid-run: stopped shows
  and editing returns. Start a graph hung the way story 05 allows — a
  node with an input neither connected nor parameterised while another is
  connected — and it runs without ever finishing; press Stop, and editing
  and save return. Change a value so the graph no longer compiles and press
  Start: the errors name the edited input and the state stays idle; set it
  back and start again — it runs, the edited definition is what compiled.
  Wire a deliberately broken graph and press Start: the compile errors show
  and the editor stays editable. Open a second tab while the long sample
  runs: it shows running, and a stop there stops it everywhere.
  DEVELOPMENT.md describes how to run it.

## Comments

- 2026-09-16 — Scope seams: this story's additions to story 11's message
  catalogue are the start and stop commands and the run state that travels
  beside the definition (15's rule, which 15's comment reserved for this
  story); the per-node event stream reaching the browser — and how much value
  data it carries — is 17, which also settles whether a browser connecting
  mid-run can attach to the live stream; systematic error surfaces are 18;
  multi-selection 20; the fizzbuzz UI mode consuming this capability is 24
  (REQ-69), inheriting 10's printing rule for its console. In the editor
  example, outputs no node consumes are discarded per story 06 — nothing here
  attaches a console printer; that is 24's wiring. The example plugins gain
  one node type whose behaviour takes a count and emits its values with a
  pause between them, and the sample the editor example ships runs it to a
  large count: without it, every node the shipped plugins build completes in
  microseconds and no run lasts long enough to observe or interrupt. The
  fizzbuzz nodes stay unlinked from the editor example — their meeting the
  editor's binary is 24's — and this keeps the story independent of 09's
  schedule. REQ-14 completes across 16 and 17: here the editor's runs are
  observed — the run state the chrome shows is driven by the events — there
  the nodes animate. (REQ-74)
- 2026-09-16 — UX calls settled here: one control that flips Start↔Stop
  rather than two buttons — the states are mutually exclusive and the flip is
  the honest state display; the control sits beside the file controls because
  run and file are the chrome's two acts on the graph as a whole; no
  interactive-mode toggle, reading REQ-59's conditional as the editor session
  itself — a second mode is ceremony for a local single-user tool; Start
  disabled on an empty definition because a run that flashes complete on an
  empty canvas is a silent non-event — the disabled control says "nothing to
  run" plainly; selection, panning, and zoom stay live while running because
  looking is not editing.
- 2026-09-16 — Why the server enforces the lock the UI shows: the browser
  holds no graph state (12), so a stale or second tab's edit must be refused
  where the definition lives; the UI disabling is the affordance, the server
  rejection is the truth, and one rule covers every editing operation,
  including 15's file ones. (REQ-24, REQ-25)
- 2026-09-16 — Why every start recompiles: keeping a compiled graph across
  edits would make "what runs" one decision away from "what is shown" — the
  divergence the server-owned-definition rule exists to prevent; compile is
  04's pure, fast transformation, so REQ-26's fast recompile holds by
  construction rather than by a caching mechanism — one of each. The visible
  consequence is the loop: stop, edit, start, and the edit is unconditionally
  what runs. (REQ-26)
- 2026-09-16 — Stop is neither an error nor a failure: 06's fail-fast
  cancellation posture is adopted for the user-triggered case — the guarantee
  is that the run ends and the outcome is told, not a frozen instant nor
  delivered in-flight values — and "stopped" is a distinct outcome so a
  user-stopped run never reads as one that went wrong. This deliberately
  narrows story 06's await guarantee: "the engine does not end a run while
  anything it wired holds an undelivered value" holds for the natural ends —
  completion and fail-fast — and the user-triggered stop is the one way a run
  ends with undelivered values abandoned. Recorded so the engine contract
  change is deliberate, not a choice between two story records. The stop also
  ends a run making no progress — story 05's never-firing input — where
  awaiting forever would otherwise be the only future.
- 2026-09-16 — The empty-definition rule is enforced on both sides: the
  disabled control is the chrome's answer, and a start arriving for an empty
  definition — a stale tab's — is answered with an error naming it, the
  state left idle. An empty graph runs and finishes immediately (06), so
  left unset the silent non-event this story rules against is reachable from
  a tab that cannot see the disabled control; this is a check truly needed
  for correct operation, consistent with the start-while-running and
  stop-while-idle answers, not an extra failure mode. (REQ-59, REQ-72)
- 2026-09-16 — Process exit stays unset, as 15 recorded: Ctrl-C mid-run or on
  an unsaved graph still loses what is unsaved, and this story adds no quit
  guard; whether the fizzbuzz binary's UI mode (24) wants one is its
  population's call, noted so it reads as a decision. (REQ-74)
