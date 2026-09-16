---
title: Core crate and inventory plugin registry
date: 2026-09-16
---

## Description

The foundation the whole product stands on: a cargo workspace whose `nodetool` core library crate carries the abstract node model and the plugin registry, and a registration path where a plugin crate contributes node type descriptors (stable id, label, icon, typed ports, grouping) purely by being linked into a binary — via `inventory`, with no manual registry to maintain and nothing in core named after any concrete node or plugin.

This is the first extension point of the product: every later story — type model, graph files, compiler, engine, editor — assumes node libraries arrive this way. Its proof is deliberately small: a runnable example binary that links a sample plugin and prints the node types the registry collected, so a plugin author can see the path work end to end before building anything on it.

## Definition of done

- From a fresh clone, the workspace builds and contains the `nodetool` core library crate holding the abstract node model and registry machinery — and no concrete plugin node types live in core. (REQ-3)
- Core provides the logic for defining a node type: a descriptor with a stable id (uuid), a default label, an SVG icon reference, and typed input/output port definitions. (REQ-5, REQ-42)
- A port declares the data type(s) it accepts as one or more type references, so union-typed ports are expressible from the start; the type model those references point at arrives with the type story. (REQ-27)
- A descriptor can carry a plugin grouping and an optional sub-group, so node libraries of any size have a home in the registry. (REQ-10)
- A separate plugin crate registers node type descriptors by linking alone — adding a node library to a binary means adding a dependency, never editing core or maintaining a registry by hand. (REQ-8, REQ-9)
- The registry collects contributions from every linked plugin, including two plugins linked side by side, and reports conflicts such as two plugins claiming the same type id instead of silently dropping one. (REQ-71)
- A runnable example binary links a sample plugin crate and prints the node types the registry collected — labels, ports with their type references, and grouping — so registration can be verified by eye; this output is the story's visible proof. (REQ-8)
- Nothing in core switches behaviour on a specific node type, name, or id; the registry treats every contribution as an opaque descriptor. (REQ-73)
- Registration and collection are covered by focused tests, the workspace is clippy-clean, and `DEVELOPMENT.md` at the repo root explains how to build, run the example, and run the tests.

## Comments

- 2026-09-16: Boundaries with neighbouring stories, so this one stays small: the scalar type set, custom type registration, and conversion declarations are story 02 — until then the example's ports carry declared type references without a populated type registry. Palette presentation of icons, colours, and grouping is story 19; custom node UI is story 21; node behaviour and stream semantics are story 05. The example binary here is a proof fixture for the registry, not the start of the utility node library (story 08) or the fizzbuzz plugin (story 09).
