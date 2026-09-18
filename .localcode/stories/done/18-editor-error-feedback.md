---
title: Editor error feedback
date: 2026-09-16
pr_id: 54
---

## Description

Stories 12–17 grew the editor capability by capability, and with each one a
promise was deferred: whatever goes wrong will be "plainly visible" — a file
that fails to load names its path (15), a failed start names its compile
errors (16), a failed run names its node (16), an edit that cannot apply is
answered with an error (11). Each report went wherever its own story's moment
put it, and two silences remained: nothing shows a problem before a run is
spent finding it, and the connection can die without the editor saying a
word. This story keeps all those promises in one place. The vision's standing
lines are "no silent states" and "the editor warns early, compile enforces"
— this story is what they look like in the editor: one visible surface every
report arrives through, warnings while the graph is still editable, and a
clear word when the connection is lost. (REQ-60)

The surface for transient reports is one mechanism, deliberately small: a
toast in the chrome, dismissed on click and on its own. Through it arrive the
reports the earlier stories already define, their content unchanged — a file
that fails to load or save (15), a failed start's compile errors (16), a
failed run's error (16), an error reply to anything the editor asked (11),
including an edit rejected because a run is on. No report is ever again
swallowed into a browser console. What toasts deliberately do not carry is
the durable truth: a toast is a happening, and happenings dismiss — the
lasting state lives on the canvas and in the chrome, where the next
paragraphs put it. (REQ-71, REQ-2)

That durable state is the early warning. After every change to the held
definition, and when a file is opened, the server recomputes the graph's
problems through the same compile a start runs (04) and holds the result as
state like the definition itself — pushed to every connection when it changes
and carried in the connect-time resync, so a second tab and a reload show the
same warnings without anyone re-asking. The problems shown are exactly
compile's, not a browser-side re-implementation with its own ideas; the
recompute rides the pure, fast compile REQ-26 makes first-class, cheap enough
to run after every edit. And it changes nothing about what the user may do: a
graph with problems edits, saves, and starts exactly as freely as a clean
one, because enforcement stays compile's at start (16) — the editor warns, it
never polices. (REQ-60, REQ-72, REQ-26)

Problems live where compile says they live. Each node a compile error names
is marked on the canvas, the message — the ports, types, and reason compile
already writes — readable at the mark. A node whose type reference this
binary never linked is story 12's inert placeholder, and its mark finally
explains why it is inert: the unknown-type error it has carried since the
file opened. Fix the problem and the mark is gone on the next recompute; a
clean graph carries nothing — no all-clear badge, no count to babysit; the
quiet canvas is the normal. When Start is pressed anyway, 16's failed-start
report arrives through the surface, and the marks on the canvas are the same
errors seen durably — one compile, one truth, two views. (REQ-71, REQ-47)

One warning the compile itself must gain, amending the home story 04's
delivered compiler gave this flagging (its record left it to the editor's
warn-early set) and so settling story 05's hand-off: the hang gate. A node
with an input neither connected nor parameterised while another input is
connected or parameterised compiles clean and then hangs its run without
error until Stop ends it (05, 16) — a trap the shipped nodes make trivial to
build and only a wasted run to discover. It becomes compile's
first non-fatal warning, naming the node and input and saying what will
happen, advisory everywhere: it never blocks a start, and a headless compile
shows it the same way, changing no outcome. The compiler's result gains
warnings beside its errors — the same function, an additive extension, no
second validation path — and the set grows only when another warning earns
its place. (REQ-60, REQ-72, REQ-74)

When a run fails, the report arrives through the surface naming the node
instance and the failure (16), and the explanation also settles where the
failure lives: story 17's failed-node mark — the persistent status this
story was told to point explanations at — carries the error message the same
way a warning mark does, so the canvas answers "what happened and where" in
one place. Both are fed from the story 07/17 event stream, no second error
channel beside it, and both go when the next start resets the canvas, the
toast having carried the message until then. (REQ-71, REQ-13, REQ-14)

Then the loss the vision calls out by name. The websocket closing — server
stopped, network gone, laptop asleep; mid-run is where it hurts and idle is
where it sulks — turns the chrome to a clear disconnected state: a banner
naming the loss and saying the editor is trying again, over a canvas that
keeps its last-known view, honestly stale. While disconnected the editor is
view-only: every server-acting gesture — editing, file open and save, start
and stop — is inert, because an edit the browser cannot deliver is a lie the
browser holds no state to back (11); looking stays live, because panning,
zooming, and selecting are the user's own view. The banner never covers the
canvas. And the run behind the lost connection is untouched by the loss —
the engine never waited on the browser (17) — though a server that was killed
took its run with it, one process one graph, and the reconnection will say so
honestly. (REQ-45, REQ-2)

Reconnecting is nobody's chore: the editor keeps trying, and when the server
returns the tab rejoins by itself through story 17's resync — definition,
file, run state, status snapshot — the banner clears, and a reconnect mid-run
reattaches to the live stream, the same path as any first connect with
nothing extra built. A first open that cannot reach the server shows the
disconnected state, not story 12's empty-canvas invitation — that invitation
stands only once a server has actually been reached. And the greeting's
versions (11) are read where they land: a page speaking the wrong protocol or
schema is told so visibly, with reloading the page as the advice, instead of
silently misbehaving against a server it no longer understands. (REQ-45,
REQ-71)

Deliberately not here: the content of any report — the compile error set is
04's, the file-failure contracts are 15's, fail-fast and the run outcomes are
06 and 16's, statuses and the reattach snapshot are 07 and 17's; palette
icons, colours, and grouping (19); multi-selection (20); custom node UI (21);
the accessibility pass over these marks, toasts, and banner (22); groups
(23); the fizzbuzz UI mode consuming all of it (24).

## Definition of done

- The editor has one visible surface for transient reports — toasts in the
  chrome, dismissed on click and on their own — and every report an earlier
  story promised "plainly visible" now arrives through it with its content
  unchanged: a file load or save failure (15), a failed start's compile
  errors (16), a failed run's error naming the node instance and the failure
  (16), and an error reply to any editor request (11), including an edit
  rejected because a run is on. No failure or error reply passes unshown into
  the browser console; each report names what and where its owning story
  defines — compile's messages name the instance uuid and type reference
  (04), a run failure the node instance and the failure (16), a file failure
  the path (15) — and each message is shown at the labelled node it names,
  label beside uuid where labels collide, so the where is the node the user
  sees rather than a bare uuid. Nothing durable lives in a toast: the lasting
  truth is the marks and the chrome state below. (REQ-71, REQ-2, REQ-74)
- After every change to the held definition, and when a file is opened, the
  server recomputes the graph's problems through the same compile a start
  runs — no second validator, no separate check the browser performs — and
  holds the result as server state like the definition: pushed whole to every
  connection when it changes and carried in the connect-time resync, so a
  second tab and a reload show the same warnings. (REQ-60, REQ-2, REQ-26,
  REQ-45)
- Each node a compile problem names is marked on the canvas, its message —
  ports, types, and reason, as compile writes it — readable at the mark; the
  compile result carries, beside each problem's message, the node instances
  the problem names, and the mark and the pushed problems state ride that
  attribution, never a parsing of the message, so marks, the failed-start
  report, and the headless printout read one structure and none of them
  scrapes the other. An unknown-typed placeholder carries its unknown-type
  error, explaining its inertness. Marks clear when the problem does, on the
  next recompute, and a clean graph carries nothing. Marks advise only: they
  disable no gesture and block no edit, no save, and no start compile allows
  — a graph with problems is saved and started as freely as a clean one; only
  compile errors refuse a start, per 16. (REQ-60, REQ-72, REQ-47)
- The compiler's result gains non-fatal warnings beside its errors — the same
  compile, an additive extension, no second validation path — carrying, with
  each problem, the node or nodes it names, so the editor places marks from
  the result's structure, not by parsing message text. Its set starts with
  exactly one member: an input neither connected nor parameterised while the
  node has another input that is connected or parameterised, the story 05
  hang gate, warned naming the node and input and the consequence; it changes
  no outcome and blocks nothing, in the editor and headless alike. New
  warnings join only when they earn their place. (REQ-60, REQ-72, REQ-74)
- A failed run's explanation lives at its location: the story 17 failed-node
  mark carries the error message the same way a warning mark does, and the
  toast reports the same error naming the node — both fed from the story
  07/17 event stream, no second error channel; both go when the next start
  resets the canvas. (REQ-71, REQ-13, REQ-14)
- A lost websocket connection is a clear, non-blocking chrome state: a banner
  naming the loss and saying the editor is trying again, over the last-known
  canvas, visibly stale. While disconnected every server-acting gesture —
  editing, file open and save, start and stop — is inert, the browser holding
  no state that could back an undeliverable edit, while panning, zooming, and
  selecting stay live. The run behind the lost connection is untouched by the
  loss; a server that was killed took its run with it, and the reconnected
  view says so honestly. (REQ-45, REQ-2)
- Reconnection is automatic and stateless to the user: the editor keeps
  trying, and on the server's return the tab rejoins through story 17's
  connect-time resync — the banner clears, a mid-run reconnect reattaches to
  the live stream, and the view is whatever the server now holds, the same
  path as any first connect with nothing extra. A first open that cannot
  reach the server shows the disconnected state rather than the empty-canvas
  invitation, and a greeting whose protocol or schema versions mismatch the
  page's puts the editor in the disconnected state naming the mismatch, with
  reloading the page as the advice, rather than proceeding against a server
  it no longer understands — the mismatch stays reported there until that
  reload, never a self-dismissing toast. (REQ-45, REQ-71)
- Focused tests cover the server side and the compiler: the compile result
  carrying warnings beside errors and, with each problem, the nodes it names;
  the hang-gate warning present exactly for a node with an
  unconnected-and-unparameterised input beside a connected or parameterised
  one and absent otherwise, advisory in every consumer; problems recomputed
  after an edit that introduces a compile error, pushed whole to every
  connection, carried in the connect-time resync, and cleared when the fix
  lands; a problem graph still accepting edits and saves, and a start compile
  allows; an error reply answered with the connection usable, per story 11's
  framing. The workspace builds and passes `cargo test` and `cargo clippy`
  cleanly, with the UI build part of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: commit a value that cannot suit
  its input and see the node marked as it commits, the message naming the
  types — fix it and the mark is gone; wire an output back into its own node
  and see the cycle marked before any run; build the hang gate — a node with
  a spare input beside a connected or parameterised one — see the warning,
  press Start anyway, and find the run hanging until Stop ends it, the
  warning having blocked nothing; press Start on a broken graph and see the
  report list each error
  naming what and where, the same errors sitting as marks on their nodes —
  save the graph anyway, then fix and start; make a node fail deliberately as
  story 17 does and see the failure reported, the failed node's mark carrying
  the explanation after the toast is gone; kill the server mid-run and see
  the banner over the stale, view-only canvas, editing inert — relaunch the
  server and watch the tab rejoin by itself, the resync showing the state the
  new server honestly holds. DEVELOPMENT.md describes how to run it.

## Comments

- 2026-09-16 — Scope seams: this story's only addition to story 11's message
  catalogue is the problems state travelling beside the definition — the
  push-on-change and connect-time-resync rule 15's comment set for successive
  state — everything else is presentation over paths that already exist.
  Content stays with its owners: the compile error set and its naming are
  04's, extended additively here with the warnings channel and each problem's
  node attribution; the file-failure contracts are 15's; the failed-start
  report, outcomes, and the run lock are
  16's; fail-fast behaviour is 06's; derived statuses, the mid-run attach,
  and the reconnect snapshot are 07 and 17's — the failed-node mark here is
  17's rendered status wearing this story's message. Palette icons, colours,
  and grouping are 19's; multi-selection 20's; custom node UI 21's; the
  accessibility floor over marks, toasts, and banner is 22's; groups 23's;
  the fizzbuzz UI mode 24's. (REQ-74)
- 2026-09-16 — UX calls settled here: toasts are happenings, canvas marks and
  chrome state are the truth — the same division as 17's events and snapshot,
  so a dismissed toast loses nothing; no problems panel or list — the canvas
  already locates every problem at its node, and a list would be a second
  view of the same truth needing its own reconciliation; marks at node level
  rather than styled wires — one marking mechanism, the message naming the
  ports finding the wire without a second one; no all-clear display and no
  count — a clean graph is the quiet normal; reconnect is automatic, no
  retry button and no give-up, because a user watching a banner wait to press
  it is ceremony for a loopback tool; the banner never covers the canvas,
  since looking is not editing.
- 2026-09-16 — Why the early problems are compile's output and not the
  browser's: the alternative is the registry and the compile re-implemented
  client-side — a second validator with its own drift from the real one, the
  parallel path the vision forbids. The server already holds the definition
  and the registry; the recompute rides 04's pure, fast compile (REQ-26)
  after every edit, and the state machinery it needs is the one every story
  since 12 has used. No manual check button: with the recompute on every
  change, a button is ceremony — problems are current by construction, the
  same argument 15 made for metadata.
- 2026-09-16 — Why the hang gate is a warning and not an error, and why it
  lives in the compiler: story 05 handed the decision here, and story 04's
  compile recorded it the other way — the editor's warn-early set, not
  compilation — a record this story revises deliberately, that doc comment
  updated in the same change; the runtime behaviour having been the only
  signal a user got. It is not an error because it is not incorrect — the
  graph compiles, the model allows
  it, and enforcement belongs at start, where 16's Stop is the exit; it is a
  warning because the run it hangs is otherwise undiscoverable except by
  spending a run. It lives in the compiler's result so the editor, the
  failed-start report, and a headless compile all say the same thing from one
  mechanism. Should a future story want the headless path to refuse such a
  graph, that is an enforcement change to make there, in the open.
- 2026-09-16 — Why editing is inert while disconnected, when the run lock's
  rule was "the UI disables, the server rejects": the same two layers, with
  the server one unreachable — the UI's disabling is the only layer left, and
  it is honest precisely because the browser holds no graph state (11).
  Nothing queues: a queued edit would be a second graph source the server
  never sanctioned, the exact divergence 11's server-ownership exists to
  prevent. (REQ-2)

- 2026-09-18: Implemented in pull request #54 (http://localhost:8080/gitea/localcode/nodetool/pulls/54).
