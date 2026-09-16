---
title: Graph files from the editor
date: 2026-09-16
---

## Description

Stories 12–14 gave the user a live graph: nodes dropped onto the canvas,
named, valued, wired and unwired — held entirely in the server's memory, a
graph that dies with the process. Nothing yet connects that graph to the file
it deserves to be. This story closes the loop with the file format story 03
built: the editor opens graph files and saves the graph back to one, so an
editor-built graph is a kept artefact and a hand-written file is an
editor-openable one — and the user can tell at a glance which file they are
editing and whether it is saved.

The outcome, as the user sees it: launch the visual binary on a graph file
and the editor opens already showing that graph; open a different file from
the editor and it replaces the graph in every open tab; edit and save, and
the file on disk is YAML the user could just as well have written by hand —
metadata current, meaning intact. After this story the editor's graph stops
being volatile: it lives in a file the user owns, named in the chrome, and
the loop — build, save, quit, hand-tweak, relaunch — needs no ceremony. The
same file serves both doors: the one the headless run (06) consumes is the
one the editor edits.

The file work is server-side, like everything that makes the graph true.
Open hands the file to the story 03 loader — structural parsing only — and
replaces the definition the server holds; save serialises that definition
through the same format. The browser holds no file state of its own: it asks,
the server answers and pushes the state that changed, and every open tab
shows the same graph and the same file name.

Loading stays structural and no more, story 03's settled line: a file whose
nodes reference types this binary never linked opens fine and renders as
story 12's inert placeholders; a node carrying no position lands on the
deterministic fallback; nothing about types, ports, or cycles is judged at
open — compile time decides (04) when a run starts (16). The editor must
never police a file the headless path would accept: one format, judged at the
same places, whichever door it enters through.

Saving is faithful, not embellished: the definition the server holds is
exactly what is written — schema version and all — with nothing invented at
save time. Positions are already current by construction: story 12's create
and move wrote them into metadata as the gestures happened, and stories 13
and 14 changed only structural fields, so "keeping visual metadata current"
is not a save-time pass but a property the editing gestures maintain as they
go — save simply honours it. A node the user never positioned is written with
no position at all; the deterministic fallback puts it in the same place next
time, so nothing needs writing.

Save always knows its target: the path the binary was launched with, the last
file opened, or nothing yet. Save writes to the current file; the first save
of an untitled graph asks for a path, and the answer becomes the graph's file
— the same ask doubles as saving the working graph elsewhere, so there is one
control and one dialog, not a second save mechanism. The chrome names the
current file (or says untitled) and shows whether the held graph has unsaved
changes since the last open or save. Opening another file, or starting a
fresh graph, over unsaved changes asks before discarding — the one guard,
placed exactly where loss is real.

Errors stay honest and cheap, per story 11's framing: a file that fails to
load leaves the held graph untouched and reports the story 03 load error,
naming what and where; a save to an unwritable path reports the failure and
changes neither the graph nor any file; the connection stays usable. The
reporting here is plainly visible in the editor; making the surfaces
systematic — toasts, panels, compile warnings, connection loss — is story
18's.

Deliberately not here: compile checking of opened files and early warnings
(04, 18); start/stop and the lock that covers file operations while running
(16); live events (17); palette presentation (19); multi-selection (20);
custom UI (21); groups in files (23); fizzbuzz's UI mode wiring this into the
product binary (24).

## Definition of done

- Launching the editor example with a graph file path opens the editor
  already showing that graph — nodes placed by their metadata positions,
  unpositioned nodes on the story 12 deterministic fallback, and nodes whose
  type references are missing from the listing rendered as inert placeholders
  with their stored labels, nothing silently dropped; launched without a path
  it starts on story 12's empty canvas. In both cases the chrome names the
  file being edited, or says untitled. The example ships a hand-written
  sample file exercising all three placements. (REQ-4, REQ-11, REQ-71)
- The editor can open a graph file from a path: the story 03 loader parses it
  structurally, the held definition is replaced, selection is cleared, and
  the new graph and file name are pushed to every connection, so all open
  tabs show the opened graph. Loading is structural only: a file referencing
  unlinked types opens with placeholders, and no type, port, or connection
  check happens at open — compile time judges (04), because the editor's job
  is to never block free editing. (REQ-11, REQ-60, REQ-45)
- The editor can save the held definition as YAML to the file being edited:
  the story 03 format with its schema version, and nodes, parameters, edges,
  and metadata carried exactly as held. Saving an untitled graph asks for a
  path, which becomes the current file; saving elsewhere re-targets the same
  way. Nothing is invented at save time: a node the user never positioned is
  written with no position, and reopening lands it identically via the
  deterministic fallback. (REQ-11, REQ-12)
- Round trip holds through the editor: open a hand-written file, save it with
  only viewing or visual edits made, and the file comes back
  meaning-identical — uuids, labels, parameter values, metadata, and edges —
  so hand-written and editor-written files stay interchangeable, extending
  story 03's guarantee across the editor's door. (REQ-11, REQ-12)
- The editor shows which file it is editing and whether the held graph has
  unsaved changes since the last open or save; a reload or a second tab shows
  the same file name and state. Opening another file, or starting a fresh
  graph, over unsaved changes asks before discarding them; a fresh-graph
  control returns the editor to an empty, untitled graph in one step.
  (REQ-2, REQ-45)
- A file that fails to load leaves the held graph and current file untouched
  and reports the story 03 load error naming what and where; a save that
  fails — an unwritable path — reports it, changing neither the graph nor any
  file; in both cases the connection stays usable. (REQ-71)
- Focused tests cover the new messages server-side: launch-with-file seeding
  the server's held definition through the story 03 loader; open replacing
  the definition and pushing to every connection; save writing the document
  the definition dumps, checked against the format's own round-trip fidelity;
  save retargeting the current file; a load failure and a save failure each
  leaving the state untouched and answered with an error per story 11's
  framing. The workspace builds and passes `cargo test` and `cargo clippy`
  cleanly, with the UI build part of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: launch the editor example on the
  sample file and see all three placements — metadata position, fallback
  position, inert placeholder; drop a node, move it, wire and value it with
  stories 12–14's gestures, save, and read the file in a text editor: the
  edits are there as clean YAML, the moved node's position in its metadata;
  relaunch on the file and see the same graph again; make an unsaved change,
  see the indicator, then open another file and be asked before it is
  discarded; try opening a deliberately malformed file and see the error name
  the file's fault with the graph untouched.

## Comments

- 2026-09-16 — Scope seams: this story's additions to story 11's message
  catalogue are open and save, plus the current-file state reaching the
  browser beside the definition it belongs to. Launching on a file simply
  seeds the server-held definition (11's state) through the story 03 loader —
  no new mechanism. The visual push shape is 12's, unchanged: an open pushes
  the whole new definition; save changes no definition state, so its reply
  suffices. Compile warnings on opened files are 18's; the run-time lock over
  file operations is 16's — open and save are edits like any other, so once
  runs exist they sit under the same edit lock (REQ-25); nothing to lock yet,
  as nothing runs yet. Fizzbuzz's UI mode (REQ-69) is story 24's, consuming
  this story's capability; the console printing there inherits story 10's
  rule. (REQ-74)
- 2026-09-16 — Why server-side paths, not browser file pickers and
  downloads: the server owns the graph (11), files live in the same
  filesystem as the binary, and the loopback single-user posture — no auth,
  no remote access — makes a path from the browser exactly as trustworthy as
  the one typed at launch; the parse boundary stays server-side either way,
  the story 03 loader the sole judge of what a file is. A picker-and-download
  model would split open from save into two mechanisms and strand saved files
  in a downloads folder the user then shepherds; a path keeps the editor
  literally editing a named file — the same file the headless run takes.
- 2026-09-16 — UX calls settled here: save always knows its target, asked for
  only when untitled, the answer becoming the current file — one dialog
  serving first-save and save-elsewhere, so no second control; a fresh-graph
  control completes the file model (launch-with-file, open, save, new) at the
  cost of one more use of the same replace path; the unsaved-changes confirm
  sits only where loss is real — replacing the held graph — and nowhere
  else; the dirty state is server-known (definition changed since the last
  open or save), so every tab agrees without browser-side bookkeeping. No
  file watching and no mtime checks: the editing session is the working
  copy, save writes it whole, and picking up hand edits means re-opening —
  watching would add a second graph source to reconcile for a workflow
  (hand-editing mid-session) the tool never encouraged. (REQ-72)
- 2026-09-16 — Why save invents nothing: story 12 settled that unpositioned
  nodes get a deterministic, view-local fallback with nothing written into
  the definition; save honours that line — the file mirrors the definition,
  and the fallback makes missing positions stable across reopen rather than
  a gap. Any save-time help (writing computed positions, reordering,
  reformatting opinion) would break story 03's round-trip contract from the
  editor side. Metadata is current by construction — 12's create and move
  wrote positions as the gestures happened, 13 and 14 touched only
  structural fields — so there is no metadata pass at save because there is
  nothing for it to do. (REQ-12)
- 2026-09-16 — Open and save are the same format, not a parallel one: the
  loader and dump are story 03's, the in-memory definition the same one the
  server holds and the compiler takes (04) — no second graph representation
  enters for the file path, matching story 11's two-serialisations-one-model
  line. Whatever schema room story 23's groups need rides the same loader
  and dump untouched. (REQ-44)
