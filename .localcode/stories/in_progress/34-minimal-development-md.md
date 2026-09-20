---
title: DEVELOPMENT.md back to a minimal guide
date: 2026-09-20
---

## Description

DEVELOPMENT.md has grown into a 537-line essay: a paragraph per crate
re-narrating what its rustdoc already specs, the palette's sorting rules, the
fizzbuzz binary's Ctrl-C guard, the samples' storylines spread across three
sections — Layout, Run the examples, Writing a plugin. A developer onboarding
file answers three questions — what is this, how do I build and test it, how
do I run it — and stops. The narrative already has its home: the crates'
rustdoc is the spec for the graph format, the compile to an executable graph,
the stream semantics, the engine lifecycle, and the server protocol, and the
`node_type!` and `data_type!` rustdocs are the plugin author's guide —
DEVELOPMENT.md itself says so while out-narrating it.

Cut DEVELOPMENT.md back to a minimal guide:

- what nodetool is — a few lines, not a tour;
- setup — the toolchains, in a list;
- build and check — the commands that exist when this story is implemented
  (DEVELOPMENT.md:127-138 today);
- run — the commands worth typing, one line each, the four command blocks
  consolidated (DEVELOPMENT.md:140-153, 372-376, 433-440, 459-462), the
  `visual` line keeping its open-the-printed-address step;
- where the specs live — one line each pointing at `nodetool::graph`,
  `nodetool::compile`, `nodetool::behaviour`, `nodetool::engine`, and
  `nodetool::server` rustdoc, plus the `node_type!` and `data_type!`
  rustdocs as the plugin-authoring guide, replacing the prose that
  duplicates them.

What a crate is, what a node type is, how groups compile away, how the
palette sorts — that is rustdoc's and the stories' business; the guide keeps
only what a newcomer needs to get the code running and find the rest.

## Definition of done

- DEVELOPMENT.md contains: a short project description, setup, build/check
  commands, run commands, and pointers to the rustdoc specs — nothing that
  restates a rustdoc spec, no per-crate narrative, no feature walkthroughs
  (REQ-70).
- A newcomer can go from a fresh clone to a passing build, test, and a
  running editor using the file alone; every command in it was run as
  written.
- The file stays a description of the project, not of any change — no story
  notes, no progress, no workarounds.
- `cargo fmt --check` and the full check suite still pass; every module or
  macro path the guide names exists and has rustdoc (`cargo doc -p nodetool`
  builds clean).

## Comments

- 2026-09-20 — No requirements section bears on this one directly; it serves
  REQ-75's "works as intended" by keeping the door readable. The guidance the
  story format gives DEVELOPMENT.md — describes the project, never a change —
  is the whole design constraint: if a sentence explains how a *feature*
  behaves rather than how to *build or run*, it belongs in rustdoc, and the
  cut moves it there or drops it.
- 2026-09-20 — No earlier story changes a command: the fizzbuzz invocation
  keeps its form through story 29's graph rebuild, and none of the UI
  stories touch the build step. If order matters at all it runs the other
  way — story 29 falsifies the crate prose this cut removes
  (DEVELOPMENT.md:60-63), so cutting first leaves nothing stale. Nothing in
  the earlier stories depends on this file's shape.

- 2026-09-20: Implementation got stuck at "implement story": opencode run failed (1). The story went back to ready to be picked up again.
