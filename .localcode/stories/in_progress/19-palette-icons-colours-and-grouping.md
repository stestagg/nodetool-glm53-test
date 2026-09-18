---
title: Palette icons, colours, and grouping
date: 2026-09-16
pr_id: 56
---

## Description

Stories 12–14 built the editor's working surface: nodes dragged out of a
plain, flat palette list, arranged, wired, and given values. Two promises
were parked on the way. Story 12 said its plain list is honest "until 19
replaces it with icons, colours, and plugin/sub-group organisation", and it
put each port's declared type in a tooltip rather than on the node — types
readable only by hovering, one port at a time. This story keeps both
promises. The palette becomes an organised, icon-carrying index of every
linked plugin's node types (REQ-10, REQ-42), and the graph itself grows the
type channel REQ-48 points at — colour and shape on ports and wires, so a
wired graph tells the user what flows where at a glance, no hovering needed.
After it, a user can find a node type among many plugins' contributions and
read the data types of their graph as they look at it.

The palette organises by plugin (REQ-10): one section per plugin, headed by
the plugin's name — the attribution story 12 printed as secondary text on
every row now lives once on its section header — and, where a plugin
declares sub-groups, a headed sub-section within it per sub-group, types
without a sub-group sitting directly under the plugin header. Types order
alphabetically within their group, plugins and sub-groups alphabetically
among themselves — a deterministic order, so every tab and every reload
shows the same palette. Rows keep story 12's compactness and gain the type's
declared icon (REQ-42); drag-and-drop onto the canvas is story 12's gesture
unchanged, working from whatever depth the row sits at.

On the canvas, nodes render their type's icon in the title bar beside the
label (REQ-42) — the one addition, the nodes staying as compact as story 12
and 14 left them (REQ-47). A node whose type reference is missing from the
listing has no icon to render and shows none, as inert as before (REQ-71).

Colour and shape inform types (REQ-48). Each data type carries a colour and
a port shape, and they show wherever that type shows: a port declaring
exactly one type renders its dot in that colour and shape, and a wire
renders in the colour of the source port it flows from. A port declaring a
union of types, and any type reference the listing does not know, render in
one neutral colour and shape — an honest "no single type to inform" rather
than a guess; the declared members remain readable in the tooltip (12) —
colour makes types visible at a glance, the tooltip still answers exactly
what is declared. A port shape is a name from the small fixed set the
editor's renderer draws — the renderer's own vocabulary, like its neutral
pair; a declared shape outside the set renders the neutral, the declarer's
to fix, as with icons. Colour never carries information alone — the shape
distinguishes beside it wherever the type shows as a port, and a wire's
colour reads against the shaped source port it flows from, leaving story 22
a contrast pass, not a rebuild.

Where the colours come from is the same path everything else takes. Core's
base scalars are declared through the same `data_type!` mechanism a plugin's
custom types are, so core declaring its scalars' colours and shapes is an
ordinary declaration change in its own type declarations — no special case:
composing the fact reads the declared colour and shape and nothing else
about the type, and no core behaviour switches on either (REQ-29, REQ-73).
A custom type's colour and shape ride the metadata map the type model
already carries — top-level presentation metadata, exactly the kind of
detail core may transport while knowing nothing else about the type
(REQ-31, REQ-33). A type that does not declare both a colour and a shape
gets the neutral. No type's colour or shape is hardcoded in the browser:
the node-type listing gains one flat fact per type reference — its colour
and shape, the neutral when the type does not declare both — composed
server-side where the metadata lives, the same additive change to story
11's catalogue that story 14's base-scalar classification was.

Deliberately not here: run-status marks, which are states not decoration
(17); error and warning surfaces (18); multi-selection (20); plugin custom
node UI, which can replace any of this presentation per type (21); the
accessibility pass over contrast, keyboard, and focus (22); groups (23); the
fizzbuzz UI mode consuming the whole (24). No palette search or filter, and
no node-header colouring — settled in the comments.

## Definition of done

- The palette groups node types by plugin: a section per plugin headed by
  the plugin's name, a headed sub-section per declared sub-group within it,
  types without a sub-group directly under the plugin header — no empty
  nesting, and the order of plugins, sub-groups, and types is alphabetical
  and deterministic, every tab and reload agreeing. The per-row plugin
  attribution story 12 added is retired — the section header carries it
  once — and dragging a type from any row onto the canvas creates the node
  exactly as before. The empty states story 12 settled (no linked types, no
  nodes yet) are unchanged. (REQ-10, REQ-46, REQ-12)
- Each listed node type's declared inline SVG icon renders in its palette
  row and in the title bar of every node of that type, beside the label;
  nodes gain nothing else and stay compact per the Blender reference. A node
  whose type reference is missing from the listing shows no icon and stays
  the inert placeholder it was. (REQ-42, REQ-47, REQ-49, REQ-71)
- Ports and wires show the data type they carry: a port declaring exactly
  one type renders in that type's colour and shape, a wire renders in the
  colour of the source port it flows from, and union-declared ports, unknown
  type references, and types that do not declare both a colour and a shape
  render in one shared neutral colour and shape. The colour and shape of a
  type come from its own declaration — base scalars from core's declarations
  of them, custom types from the declaring plugin's metadata map — and the
  graph definition and file format carry nothing new for any of it; the
  declared-type tooltip remains. Colour is never the only signal: a port
  shows the shape beside the colour, and a wire's colour reads against the
  shaped source port it flows from. (REQ-48, REQ-29, REQ-31, REQ-33, REQ-27)
- The node-type listing carries each type reference's colour and shape as
  one flat fact, computed server-side from the type declarations and added
  to the catalogue per story 11's framing, so the browser holds no
  type-name, colour, or per-type shape table — it renders the declared
  colour verbatim and the declared shape from the small closed shape set it
  ships, taking the neutral for a shape name it does not know. No type's
  colour or shape is hardcoded: the neutral pair and the shape vocabulary
  are the browser's own, the neutral the server composes for a type that
  does not declare both is that same pair, and there is one neutral
  everywhere — the same one-fact pattern as story 14's base-scalar
  classification. (REQ-73, REQ-48)
- Focused tests cover the listing server-side: the colour and shape fact
  matching a base scalar's and a custom type's declarations, the neutral
  fact for a type that does not declare both a colour and a shape, and for
  a type reference the registry does not know, and the listing still
  matching the registry per story 11's tests otherwise. The workspace builds
  and passes `cargo test` and `cargo clippy` cleanly, with the UI build part
  of the check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: open the editor example — it
  links more than one plugin — and see the palette in sections, one per
  plugin with its name heading and the shapes plugin's 2d and 3d
  sub-sections headed inside it, icons on every row; drag two nodes of
  different types out and see their icons in the title bars; wire a String
  output to a String input and see the port dots and the wire in String's
  declared colour and shape, differing in both from the wire between an
  integer-declared pair — the example plugins gain the singly-declared
  integer ports this proof needs — and the circle's union-declared radius
  input sitting in the neutral shape; reload the page and open a second tab
  — same sections, same order, same colours.

## Comments

- 2026-09-16 — Scope seams: this story changes no operation messages and no
  editing behaviour; its only protocol addition is the colour-and-shape fact
  on the listing, additive per story 11's framing exactly as story 14's
  base-scalar classification was. Run-status rendering is 17's — a running,
  failed, or completed node is marked by status, not by this story's type
  channel, and the two never share a visual. Toasts, warning marks, and the
  disconnected banner are 18's; multi-selection 20's; a plugin replacing any
  of this presentation for its own types is 21's; the contrast, keyboard,
  and focus pass over everything this story renders is 22's; a group node's
  palette entry is an ordinary declared type here, nothing group-specific
  until 23. (REQ-74)
- 2026-09-16 — UX calls settled here: palette sections are always open —
  scroll, not accordions, nothing to collapse at the scale the editor ships
  at — and alphabetical ordering beats registry order, which follows link
  order and would differ across binaries for no user benefit; the palette
  gains no search or filter field: REQ-10's grouping is the navigation aid,
  and a filter box is a control to add only when palette size asks for it;
  no node-header colouring — the label and icon already identify a node, and
  a second colour channel would compete with the type channel on compact
  nodes (REQ-47); palette rows carry no colour — colour is the data-flow
  channel on ports and wires, the palette's job is finding types, and a node
  type has no single colour under this scheme.
- 2026-09-16 — Why colour and shape ride the type declarations: the type
  model already carries a metadata map of plain, browser-serialisable values,
  and core's base scalars are declared through the same mechanism a plugin's
  custom types are — so core giving its scalars colours is an ordinary
  declaration change, core interpreting nothing it does not already transport
  (REQ-73), and a plugin's colour is top-level presentation metadata, the one
  kind of custom-type detail core may carry (REQ-31, REQ-33). Composing the
  flat fact server-side keeps the browser free of hardcoded type names — the
  same second-registry argument story 14 made for the base-scalar fact: a
  colour table in JS would silently rot the day the base set grows.
- 2026-09-16 — Wire colour follows the source port's declared type, not the
  connection's resolved type: resolution is compile's (04), edits do not
  compile, and inventing a second, partial resolution for rendering would be
  exactly the parallel path the vision forbids. A union-declared source port
  has no single type to inform and takes the neutral, its members still
  readable in the tooltip; the graph file carries nothing for any of this —
  colours and shapes live with the declarations, so a hand-written file and
  an editor-built one render alike.
- 2026-09-16 — Icons render as their plugin declares them: a plugin is
  build-time linked code, not untrusted input — the vision's untrusted
  boundary is graph files and websocket messages, and icons ride the
  registry listing, which the binary itself composes from its own linked
  declarations. A malformed or empty icon is that plugin's declaration to
  fix, not an input to sanitise.
