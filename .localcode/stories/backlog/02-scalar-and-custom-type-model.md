---
title: Scalar and custom type model
date: 2026-09-16
---

## Description

Core needs a shared vocabulary of types before anything can be wired together: every port on every node refers to a type, and the compiler (a later story) will decide which connections are legal and where a trivial conversion is needed. This story gives the project that vocabulary.

Core ships a base set of scalar types aligned with Rust's own primitives — the signed and unsigned integers, the two floats, `bool`, and `String` — so the everyday cases need no plugin at all. Alongside them, the registration path opens for plugins: a plugin crate can contribute its own custom types, each identified by a stable uuid and name, optionally declaring cheap conversions to other types, and carrying whatever additional metadata the plugin needs. Custom types stay opaque to core: beyond the descriptor (id, name, declared conversions, metadata), core neither knows nor cares what a custom type is, what its values look like, or how they are serialised or rendered — that remains the plugin's business, exercised later through plugin-provided node UI.

The trivial conversions the project promises — `i16`→`i32`, `f32`→`f64`, and the slightly looser `i32`→`f64` — are declared by the base scalars themselves, through the same declaration mechanism a plugin would use, so they are an instance of the general capability rather than a core special case. Deliberately, no automatic conversion to `String` is declared anywhere: turning values into strings is the explicit Format node's job.

For the user — today the plugin author — the outcome is: define a new data type in your plugin, link your crate, and the type exists in the system with its identity, metadata, and conversions, without touching core.

## Definition of done

- Core ships the base scalar set — `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`, `String` — present in the registry without any plugin contributing anything (REQ-29).
- Every type, core scalar or plugin custom, is carried by one descriptor: a stable uuid, a name, optional conversion declarations, and plugin-supplied metadata; there is no second, parallel notion of a type (REQ-31).
- A plugin can register additional custom types through the same inventory-based registration nodes use — uuid, name, optional conversion declarations to other types, and arbitrary metadata — without modifying core (REQ-30, REQ-31, REQ-8).
- The base scalars declare the promised trivial conversions — `i16`→`i32`, `f32`→`f64`, `i32`→`f64` — through the same declaration mechanism custom types use, so the mechanism rather than a core special case makes them work; nothing auto-converts to `String` (REQ-35, REQ-36).
- Custom types are opaque to core: beyond name, id, declared conversions, and metadata, no core code inspects a custom type's values, and core treats custom types only through their descriptors (REQ-32, REQ-33).
- Registering a type whose uuid or name is already taken reports an error rather than silently replacing or ignoring it (REQ-71).
- The example binary from the core story also lists the registered types — the base scalars plus an example plugin's custom type with its metadata and declared conversions — so identity, registration, and declared conversions are verifiable from a terminal run; focused tests cover the base set's presence, custom registration, conversion declarations, and the duplicate-registration error (REQ-74).

## Comments

- 2026-09-16: Boundaries with neighbouring stories, so this one does not creep: how a connection's type is *resolved* across unions, and where an applied conversion appears in the graph model, is the compiler story (04). The value representation values travel on, and the stream API, is the node authoring story (05). Plugin-defined ser/de and custom UI access to custom type data lands with the plugin custom UI story (21) — this story only guarantees the descriptor carries what that will need. Union-typed ports (REQ-27) build on this vocabulary but are not part of this story.
- 2026-09-16: `i128`/`u128` and `char` are deliberately not in the base set — smallest useful set. The numeric primitives listed are what "supports all numeric types" (REQ-62) will need; adding a scalar later is an ordinary registration, not a core change.
- 2026-09-16: Free-form metadata on the descriptor is the hook later stories use for colour/icon-by-type in the palette (REQ-48) without a schema change now.