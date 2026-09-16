---
title: Scalar and custom type model
date: 2026-09-16
pr_id: 19
---

## Description

Core needs a shared vocabulary of types before anything can be wired together: every port on every node refers to a type, and the compiler (a later story) will decide which connections are legal and where a trivial conversion is needed. This story gives the project that vocabulary.

Core ships a base set of scalar types aligned with Rust's own primitives — the signed and unsigned integers, the two floats, `bool`, and `String` — so the everyday cases need no plugin at all. Alongside them, the registration path opens for plugins: a plugin crate can contribute its own custom types, each identified by a stable uuid and name, optionally declaring cheap conversions to other types — each pairing the target type with the function that performs the conversion — and carrying whatever additional metadata the plugin needs. Custom types stay opaque to core: beyond the descriptor (id, name, declared conversions, metadata), core neither knows nor cares what a custom type is, what its values look like, or how they are serialised or rendered — that remains the plugin's business, exercised later through plugin-provided node UI.

The trivial conversions the requirements name — `i16`→`i32`, `f32`→`f64`, and the slightly looser `i32`→`f64` — are declared by the base scalars themselves — each declaration pairing its target type with the converting function — through the same declaration mechanism a plugin would use, so they are an instance of the general capability rather than a core special case; widening that set later is an ordinary declaration, not a core change. Deliberately, no automatic conversion to `String` is declared anywhere: turning values into strings is the explicit Format node's job.

For the user — today the plugin author — the outcome is: define a new data type in your plugin, link your crate, and the type exists in the system with its identity, metadata, and conversions, without touching core.

## Definition of done

- Core ships the base scalar set — `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`, `String` — present in the registry without any plugin contributing anything (REQ-29).
- Every type, core scalar or plugin custom, is carried by one descriptor: a stable uuid, a name, optional conversion declarations — each naming the target type's id and carrying the function that performs the conversion, supplied by whoever declares it (core for the scalars, the plugin for its own types) — and plugin-supplied metadata; there is no second, parallel notion of a type (REQ-31).
- A plugin can register additional custom types through the same inventory-based registration nodes use — uuid, name, optional conversion declarations to other types (each naming the target type's id and carrying the converting function), and arbitrary metadata — without modifying core (REQ-8, REQ-9, REQ-30, REQ-31).
- The base scalars declare the named trivial conversions — `i16`→`i32`, `f32`→`f64`, `i32`→`f64` — through the same declaration mechanism custom types use, registered and queryable from the registry through the same API a plugin's declarations use, so the compiler (04) later applies them without core special-casing; nothing auto-converts to `String` (REQ-35, REQ-36).
- Conversion declarations are storable before their target type has registered — `inventory`'s collection order is unspecified, so a target can arrive after the type declaring the conversion, even from another plugin crate — with targets resolved once every crate has contributed, and a declaration whose target never registers is reported (REQ-71), not silently dropped.
- Custom types are opaque to core: beyond name, id, declared conversions, and metadata, no core code inspects a custom type's values, and core treats custom types only through their descriptors (REQ-32, REQ-33).
- Registering a type whose uuid or name is already taken reports an error rather than silently replacing or ignoring it (REQ-71).
- The example binary from the core story also lists the registered types — the base scalars plus an example plugin's custom type with its metadata and declared conversions — so identity, registration, and declared conversions are verifiable from a terminal run; focused tests cover the base set's presence, custom registration, conversion declarations, and the duplicate-registration error (REQ-29, REQ-30).

## Comments

- 2026-09-16: Boundaries with neighbouring stories, so this one does not creep: how a connection's type is *resolved* across unions, and where an applied conversion appears in the graph model, is the compiler story (04). The value representation values travel on, and the stream API, is the node authoring story (05); a conversion declaration carries the converting function, but its signature — written against runtime values — is that story's call, while this story fixes the descriptor's shape, so 04 consumes it rather than reworks it. Plugin-defined ser/de and custom UI access to custom type data lands with the plugin custom UI story (21) — this story only guarantees the descriptor carries what that will need. Union-typed ports (REQ-27) build on this vocabulary but are not part of this story.
- 2026-09-16: `i128`/`u128` and `char` are deliberately not in the base set — smallest useful set. The numeric primitives listed are what "supports all numeric types" (REQ-62) will need; adding a scalar later is an ordinary registration; whether numeric-generic nodes (REQ-62) pick it up is for the node authoring story (05) to settle.
- 2026-09-16: REQ-35's list of trivial conversions is an exemplar ("i.e."), not an exhaustive promise; the base set deliberately declares only the three named. Widening that set later is an ordinary declaration — like adding a scalar — not a core change.
- 2026-09-16: Free-form metadata on the descriptor is the hook later stories use for colour/icon-by-type in the palette (REQ-48) without a schema change now. The metadata is serialisable data — a JSON-style map of plain values, not an in-process-only handle such as `Box<dyn Any>` — so it can later reach the browser (story 19) without a descriptor schema change, and the terminal listing of it stays trivially printable.

- 2026-09-16: Implemented in pull request #19 (http://localhost:8080/gitea/localcode/nodetool/pulls/19).
