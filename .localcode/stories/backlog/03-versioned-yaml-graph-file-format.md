---
title: Versioned YAML graph file format
date: 2026-09-16
---

## Description

Graphs are plain data files: a graph user opens a text editor, writes the graph
by hand, and the same file later loads headless or in the visual editor. This
story settles the concrete YAML document shape — a schema `version`, a list of
nodes, and a list of edges — and delivers the load/dump machinery in core plus
a short format reference document, so the format is genuinely hand-writable
before any UI exists (REQ-11).

The document shape:

```yaml
version: 1
nodes:
  - id: 018f3c2e-...        # instance uuid, unique in the file (REQ-41)
    type: fizzbuzz.counter  # node type reference, opaque string at this layer
    name: Counter 1..10     # optional user label override (REQ-40)
    position: {x: 120, y: 80}   # optional visual metadata (REQ-12)
    metadata: {}            # optional free-form visual/arrangement extras
    parameters:             # optional values for unconnected inputs,
      start: 1              # keyed by input port name, as YAML-native values
edges:
  - from: {node: 018f..., port: out}
    to:   {node: 51ab..., port: in}
```

The split the vision left open is settled: **parameters** hold values typed
inline for inputs that have no connection — the same stored value the editor's
inline field and sidebar edit later; **metadata** holds everything about how a
node is arranged or displayed — `position` now, with free-form keys so the
editor can add visual extras later without format churn. Nothing about a
node's appearance lives in parameters, nothing about a value lives in metadata.

Deliberately *not* checked here: whether a type reference exists, whether two
edges feed one input, whether a parameter suits its port, whether the graph is
acyclic. Those are compile-time concerns (story 04); the editor warns early
(story 18). Parsing builds a registry-agnostic graph model — it validates only
what the model itself needs (well-formed YAML, known schema version, unique
instance ids, edge endpoints naming nodes that exist in the file), so this
story does not depend on the registry or type model stories (01, 02) being
done, and core stays abstract: the model holds parameter values as parsed
values without interpreting them (REQ-73).

## Definition of done

- A format reference document in the repo describes every field of the schema
  above — required/optional, meaning, examples — enough that a graph user can
  hand-write a valid file with a text editor and no UI. (REQ-11, REQ-12)
- A hand-written file in the documented shape loads into core's graph model:
  nodes with instance uuid, type reference, optional name override, optional
  parameters and visual metadata; edges as output→input pairs, each edge
  naming exactly one input, with an output free to feed any number of edges
  (fan-out is just several edges). (REQ-41, REQ-40, REQ-12, REQ-20, REQ-21)
- Dumping a loaded graph to YAML and reloading it yields the same graph, and
  a dumped file stays valid under the format reference — hand-written and
  editor-written files (story 15) are the same format, not two dialects.
- A malformed file is refused with a clear, specific error — what is wrong
  and where (bad YAML, unknown version, duplicate instance id, edge naming a
  node not in the file, a structural field of the wrong kind) — and never a
  panic: files are untrusted input at the parse boundary. (REQ-71)
- A file whose `version` is unknown or missing is refused with a message
  naming the supported version, rather than guessed at; a missing `version`
  is treated as unknown, not as version 1. (REQ-11)
- A structurally valid file loads even when it is semantically odd — type
  references nothing recognised, two edges feed one input, a parameter sits on
  a connected input, values of any YAML-native kind — because enforcement is
  deferred to compile time, not duplicated here. (REQ-72, REQ-60)
- An example graph file shaped like the eventual fizzbuzz graph is in the
  repo, hand-written per the format reference, and loads — ready to be used
  by the headless run and editor stories that follow. (REQ-61, REQ-68)

## Comments

- 2026-09-16: The parameters-vs-metadata open question from the vision is
  settled as described above; `position` is optional so hand-writers can omit
  it — consumers decide their own default placement (the editor stacks such
  nodes visibly rather than rejecting the file).

- 2026-09-16: Custom data types stay out of the format's vocabulary: parameter
  values are held as parsed values, opaque to this layer. How a plugin's
  custom-type inline value is serialised is that plugin's concern (REQ-32,
  REQ-34) and lands with the type model and custom UI stories.

- 2026-09-16: Groups need no syntax reserved now. A group is a node type whose
  behaviour is a subgraph; the flat node list with string type references
  already expresses "a node that is a group". Story 23 extends the format when
  group definitions become concrete, with a version bump if the shape changes.

- 2026-09-16: Whether an applied trivial conversion appears explicitly in the
  file is story 04's open question. The edge entry has room for an optional
  annotation if that answer needs one, without breaking this shape.
