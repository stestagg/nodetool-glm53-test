---
title: Scalar and custom type model
date: 2026-09-16
---

## Description

Story 01 lets a plugin describe node ports that carry type references; this
story creates what those references point at. Core gains the type model: a
registry of data types, seeded with a base set of scalars aligned with Rust's
own types, and open to plugin-defined custom types arriving through the same
link-and-go inventory path as node types.

For a plugin author this is the moment the type system becomes real. A plugin
declares a custom type with a uuid, a name, free-form top-level metadata, an
optional set of declared conversions to other types, and optional ser/de hooks
for its data — and core treats it as opaque beyond that top level. The payload,
its serialisation, and any rendering stay entirely in the plugin; core never
learns what is inside. Core's scalars declare the trivial conversions (the
requirements name i16→i32, f32→f64, and i32→f64) so that, from the compiler
story onward, a connection across one of those conversions simply works;
anything not declared never converts implicitly, and conversion to string is
never a type behaviour — that is an explicit Format node's job.

The visible proof follows story 01's pattern: the example binary's output now
also shows the type registry — the core scalars, the sample plugin's custom
type as core sees it (name, id, metadata, declared conversion targets), and
the sample plugin's port references resolving to real registered types.

## Definition of done

- Core's type registry ships populated with the base scalar set aligned with
  Rust's types — `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`,
  `f64`, `bool`, `String` — each carrying a name and a stable uuid, visible in
  the example's output. (REQ-29)
- Core's scalars declare their trivial conversions explicitly: at least the
  requirements' named set (i16→i32, f32→f64, i32→f64) plus the same-kind
  widenings consistent with that policy, so the declarations the compiler will
  honour already exist. (REQ-35)
- The type machinery performs no conversion that is not declared, and no
  conversion to string is registered anywhere in core; turning a value into
  text is always an explicit node's job. (REQ-36)
- A plugin registers custom types purely by linking, through the same
  inventory contribution path as node types, with no manual registry to
  maintain and nothing in core aware of any concrete custom type. (REQ-30,
  REQ-9)
- A custom type descriptor carries a uuid, a name, optional conversion
  declarations targeting other registered types, and free-form top-level
  metadata. (REQ-31)
- A custom type may declare ser/de hooks for its data where the data supports
  it; the hooks are registered with the descriptor for later consumers
  (inline parameter values in graph files, custom node UI). (REQ-34)
- Core's entire view of a custom type is its top level — name, id, metadata,
  declared conversion targets: the example output shows exactly that and
  nothing more, and the registry works with zero custom types registered as
  happily as with several. (REQ-32, REQ-33)
- A port's type reference from story 01 — including each entry of a
  union-typed reference — resolves against the registry by name or uuid; the
  example prints the sample plugin's ports resolved to their registered
  types, and a reference to an unregistered type is reported as not found
  rather than guessed at. (REQ-71)
- Two contributions claiming the same type uuid or the same name — across
  plugins or within one — are reported clearly (which id/name, which
  plugins), never silently dropped. (REQ-71)
- Nothing in core switches behaviour on a specific type: the registry and
  lookup treat every type as an opaque descriptor, scalars and custom types
  alike. (REQ-73)
- Focused tests cover registration, reference resolution, each declared
  conversion producing its declared target type when applied to a value,
  ser/de hook registration, and conflict reporting; the workspace stays
  clippy-clean, and `DEVELOPMENT.md` covers running the extended example.

## Comments

- 2026-09-16: Boundaries with neighbouring stories: this story populates what
  story 01's port references point at. Resolving a *connection's* concrete
  type across unions, honouring declared conversions at compile time, turning
  a literal parameter into a typed value, and whether an applied conversion
  appears in the graph model or the file are all story 04 — nothing here
  negotiates or applies conversions between connections. That a custom type's
  inline parameter value is deserialised through the registered ser/de hook
  follows from the descriptor shape here and is exercised by story 04.

- 2026-09-16: The scalar list above is the settled baseline, chosen so REQ-62's
  "all numeric types" has something to mean; `usize`/`isize` are deliberately
  out — nothing in the requirements or vision calls for them, and the set
  stays small. Narrowing and signed→unsigned conversions are not declared
  trivial. How a node definition expresses a generic numeric port (a Counter
  over every numeric type) is story 09's open question.

- 2026-09-16: REQ-34's second half — accessing the data in a custom node UI —
  is story 21; this story only provides the registered hooks. Story 03 already
  points the serialisation of custom-type parameter values at this story and
  the custom UI story.

- 2026-09-16: Presentation hints a plugin wants for its types (colour, shape)
  ride in the free-form metadata; how the palette presents types is story 19.
  Core stays out of it. (REQ-48, REQ-73)
