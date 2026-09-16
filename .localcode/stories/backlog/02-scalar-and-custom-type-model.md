---
title: Scalar and custom type model
date: 2026-09-16
---

## Description

The type model is the vocabulary every connection in a graph speaks, and this
story populates it: the base Rust-aligned scalar set every graph can rely on
out of the box, and the extension point plugin authors use to introduce their
own data types — each identified by a stable uuid and a unique name, carrying
top-level metadata and optional conversion declarations, opaque to core beyond
exactly that.

Two behaviours follow for the people using graphs. Trivial conversions are
declared, not improvised: `i16`→`i32`, `f32`→`f64`, `i32`→`f64` and their
kin simply work when the compiler resolves a connection, while nothing
converts silently to `String` — string formatting stays the Format node's
explicit job. And named type groups (starting with `numeric`) let a node
accept a whole family of types with one port reference, so a counter or
comparison node never has to be duplicated per type; pinning a group
reference down to one concrete type is the compiler's job, not this story's.

The proof is an example binary that prints the collected type registry — the
scalar set with its groups and declared conversions, plus a sample plugin's
custom type — so a plugin author can see, by eye, both halves of the model
working: the scalars they get for free, and their own type arriving through
linking alone.

## Definition of done

- Core's type registry comes pre-populated with the base scalar set — `i8`,
  `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`,
  `String` — each with a stable uuid and unique name, so node descriptors and
  graph files can reference these types with no plugin present. (REQ-29)
- Every type carries the top-level metadata core and the editor may show —
  id, name, and optional plugin-supplied extras — and nothing more about
  custom types is visible to core, the graph model, or the editor. (REQ-31,
  REQ-33)
- A plugin registers additional custom types by linking alone, contributing a
  type descriptor (uuid, name, metadata, optional conversion declarations)
  into the same registry the scalars live in — one registry, no second
  mechanism, no core changes. (REQ-30, REQ-9, REQ-8)
- Custom type data stays opaque: core holds custom types as descriptors only,
  and a plugin may attach serialisation hooks (ser/de) to its own descriptor
  for its own node UI to use later, stored by core without interpretation.
  (REQ-32, REQ-34)
- Trivial conversions are declared on the types: the scalar set ships with
  one-way declarations for widening integer conversions, integer→float, and
  `f32`→`f64` — covering every pair REQ-35 names — and the registry exposes
  lookup of the declared conversion between two types, the API the compiler
  will consume when resolving connections. (REQ-35)
- The declared conversion matrix contains no conversion to `String`: turning
  a value into text is always an explicit node, never a silent adaptation.
  (REQ-36)
- The type model supports named type groups — the scalars include a
  `numeric` group — and a port can reference a group as well as concrete
  types, so a node can accept "all numeric types" with one reference and no
  per-type duplication; how a group reference is resolved to a concrete type
  is left to the compiler story. (REQ-27, REQ-2)
- A duplicate type id or name at collection time — including a custom type
  clashing with a scalar's name — is reported as an error, never silently
  dropped or overridden. (REQ-71)
- Nothing in core switches behaviour on a concrete custom type or its data;
  custom types flow through registration, lookup, and the example output as
  opaque descriptors. (REQ-73, REQ-33)
- An example binary prints the collected type registry — scalar names, ids,
  groups and declared conversions, plus a sample plugin's custom type with
  its metadata and conversion declarations — verifiable by eye; registration,
  conflict reporting, and conversion lookup carry focused tests, the
  workspace stays clippy-clean, and `DEVELOPMENT.md` explains how to run the
  example and the tests.

## Comments

- 2026-09-16: This story settles the vision's open question on generic
  numeric ports, via named type groups (`numeric`). That is the mechanism
  story 09 later uses so its Counter and Condition nodes accept all numeric
  types (REQ-62, REQ-63) without duplication; those requirements stay with
  story 09, which is also where compile-time negotiation of a group reference
  meets real nodes. "Type groups" are data-type classes — distinct from
  palette sub-groupings of nodes (REQ-10, story 19) and from graph
  groups/subgraphs (story 23).

- 2026-09-16: Boundaries with neighbouring stories: the registry machinery
  and node descriptors are story 01 — this story adds type contributions to
  that one registry. Inserting conversion adapters into connections, how
  unions and declared conversions participate in resolution, and whether an
  applied conversion appears in the graph file are story 04's open questions.
  The runtime value representation and when conversions are applied as values
  flow belong to the engine stories (05, 06). Palette presentation by type —
  icons, colours — is story 19; rendering a custom type in a custom node UI
  using the ser/de hooks is story 21. The scalar set above is fixed here;
  extending it later is additive and needs no rework.
