---
title: Core crate and inventory plugin registry
date: 2026-09-16
depends: []
---

## Description

Nodetool starts as a cargo workspace whose centre is the abstract `nodetool` core library crate: it knows what a node *type* is, but contains no node types of its own. Node libraries are ordinary Rust crates that declare their node types — label, icon, grouping, typed input/output ports — and are linked into a binary. Linking is the whole integration step: `inventory` collects the declarations at static-initialisation time, so there is no registry to maintain by hand and no second mechanism beside it.

The user of this story is the plugin author. After it, they can write a small crate depending only on `nodetool`, declare node types, link it into any binary, and see those types through core's registry — aggregated across plugins and separable by plugin and sub-group — without ever touching core. A small example binary in the workspace proves the mechanism end to end, and doubles as the first demonstration that the core/plugin boundary is real: with no plugin linked, core contributes nothing.

## Definition of done

- The repository is a cargo workspace containing the `nodetool` core library crate, and the core crate contains no node type implementations: with no plugin linked, the registry enumerates no node types. (REQ-3)
- A plugin author declares a node type in an ordinary crate depending only on `nodetool`, giving it a default label, an svg icon, a plugin grouping with optional sub-groups, and named input and output ports that each carry a declared type reference. (REQ-5, REQ-10, REQ-42)
- Declaration is the only registration step: statically linking plugin crates into a binary makes every declared node type discoverable through core's registry via `inventory`, aggregated across plugins and queryable so contributions can be separated by plugin and sub-group. No second registry or manual bookkeeping exists. (REQ-8, REQ-2)
- A small example binary in the workspace, linked against at least two example plugin crates, lists the node types it picked up — for each, its plugin group, sub-group where present, label, and ports. Running it (per DEVELOPMENT.md) is the visible proof of the mechanism. (REQ-8, REQ-10)
- Registry behaviour — aggregation across crates, separation by plugin and sub-group, empty without plugins — is covered by focused tests, and the workspace builds and passes `cargo test` and `cargo clippy` cleanly.
- DEVELOPMENT.md at the repository root describes how to build the workspace, run the example, and run the checks.

## Comments

- 2026-09-16 — Scope seams, so this stays small and nothing arrives early: port type references are carried as declared, opaque data — the scalar/custom type model, conversions, and unions are story 02, and compile-time resolution is story 04. Node descriptors here are pure data; the behaviour/authoring API and stream semantics are story 05, so a declared type has no runtime behaviour yet. Instance-level things (instance uuids, parameter values, user-overridden labels) belong with the graph file format, story 03. The descriptor and this single registration path are what later stories extend (custom types, custom UI, groups) — leave room, add nothing speculative. (REQ-2, REQ-9, REQ-74)
- The example plugin crates are throwaway demonstrations inside the workspace, not the fizzbuzz plugin — that arrives with its own stories, and core must not gain anything that exists only to serve it. (REQ-61 to REQ-69, REQ-73)
