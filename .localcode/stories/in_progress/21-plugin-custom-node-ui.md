---
title: Plugin custom node UI
date: 2026-09-16
pr_id: 58
---

## Description

Stories 12–20 gave every node a default rendering generated from its type
definition, and two of them parked a promise while doing it: story 17 said
values of plugin custom types animate the flow "but carry no invented
content — their rendering is plugin territory (21)", and story 14 excluded
custom-type-only inputs from the sidebar, their editing likewise "plugin
territory via 21". The vision goes further than both: a plugin may supply
custom UI for a node type, owning that UI's data handling, and a custom
type's serialisation and rendering are entirely the plugin's concern,
"reachable by the plugin inside its own node UI". What has been missing is
the path: how a plugin's UI is declared, how it reaches the browser, and
what it may own once there. Story 11 left that path a named seam — "custom
UI assets reach the browser served by this HTTP server; the plugin-facing
mechanism is story 21's, built on this seam" — and this story builds it.
The user here is the plugin author first: after this story, a plugin can
ship its own node UI and its own type rendering the same way it ships
nodes, types, and icons — by declaring them — and the graph user gets the
result as a richer node on the same canvas, with every gesture still
behaving exactly as it does on a default node.

The settlement, and why:

- **One mechanism, two attachment points.** A plugin declares UI through
  the same `inventory` path it declares everything else, at two points:
  *node-type UI* — a bundle that replaces the default node rendering for
  one node type — and *type-value UI* — a component plus a Rust
  serialisation function registered per data type, so values of that type
  display wherever values display (story 17's per-port readouts) and cross
  to the browser in the form their serialiser produces, available to any
  node UI that wants them. Two points because they answer two different
  questions — "how does this node look and edit?" and "what does a value
  of this type look like on screen?" — but one
  mechanism carries both: declared assets, served the one way, loaded the
  one way. A serialiser core calls generically is how story 17's
  contentless default gets content without core ever learning what is
  inside a custom type; a type registered with no serialiser crosses
  exactly as 17 shipped — the default is the absence of a declaration, not
  a second path. The serialisation is one-directional, display only: there
  is no deserialiser, because nothing travels the other way — parameters
  are plain scalars, values flow on wires, and the browser never sends a
  stream value back. Inventing a round trip for display would be
  speculative generality.

- **Assets ride story 11's seam.** A plugin's UI bundle is embedded in its
  crate at build time — plugins are statically linked code, and their
  assets travel with them — and the HTTP server serves it under a
  per-plugin path, exactly the "bulk things travel as assets, not
  messages" rule story 11 settled. The browser learns where from flat
  facts on the listings: the node-type listing carries, per type
  reference, its UI entry asset when one is declared; the type facts carry
  the same per data type. No fact, no custom UI. Bundles load lazily —
  when the first node of that type is on the canvas or the first value of
  that type displays — so a palette full of plugin types costs nothing
  until used; until a bundle arrives, the node renders by the default
  class, which is usable by design (REQ-38's promise, still standing). The
  same additive-fact pattern stories 14 and 19 established; the browser
  hardcodes no plugin, no type, no asset path.

- **What custom node UI owns, and what it never takes.** The shell stays
  editor-rendered — the ports, and the title bar with its label and icon
  (19) — and with it every gesture: wiring (13), moving, selecting,
  deleting, panning all behave on a custom node exactly as on a default
  one, because the gestures land on structure the editor draws. The
  plugin's component owns the body between title bar and ports: it
  receives the node's definition slice — label, parameters, connected
  inputs — and the live display state — the run state beside the status
  marks and per-port latest values, serialised — and renders the node's
  content as it pleases. This is the line that keeps the canvas coherent
  — a user should never have to relearn a gesture because a node looks
  different — and it is also what keeps the story small: the plugin buys
  content, not a second canvas. Editing from inside the plugin's UI rides
  the existing operation messages — label and parameter edits, the same
  ones the sidebar and inline fields issue — through small helpers the
  editor hands the component; there is no second editing path, and story
  16's server-enforced lock rejects a plugin-issued edit while running
  identically to any other; the run state in the contract lets the
  plugin's own controls sit inert while a run is on, the rejection
  remaining the backstop for a stale attempt.

- **The sidebar stays generic.** Story 14's comment handed "custom-type
  rendering in the sidebar" here; the settlement is that there is nothing
  to hand: the sidebar keeps its generic role for every node — label,
  read-only identity, scalar parameter fields per 14's rule — and a
  custom-type-only input has no field there, as before. A plugin wanting a
  richer editing surface for its own data renders it in its own node UI,
  where its data already is. A second, plugin-authorable sidebar surface
  would double the UI contract for one consumer today; if one arrives, it
  is designed then, in the open.

- **Contract version and failure, both visible.** The component contract
  the editor offers plugin UI is versioned, and the version travels with
  the listing facts, so a bundle built against a contract the editor does
  not speak is known at load, not half-rendered. A bundle that fails to
  load — a missing or unloadable asset, a wrong contract version — or a
  component that fails after loading, such as one that throws while
  rendering the live state, is reported through story 18's surfaces and
  the failed bundle's attachment point falls back on its own — a node-type
  UI to the default rendering, a type-value UI to 17's contentless
  readout, never the one taking the other down: usable regardless, never
  a silent blank, and the report names the type so the plugin author can
  find their mistake. The contract itself — what a component receives,
  what it may issue, its version, and the floor its author must meet — is
  documented for plugin authors, as the declaration macros' rustdoc is
  today, so a bundle can be built against it and the version names
  something checkable. Plugin UI is build-time linked code — the same
  trust story 19 gave icons — so there is no sanitising ceremony around
  it; a plugin's own malformed UI is that plugin's declaration to fix.

Deliberately not here: groups and their collapsed node UI (23 — a group's
palette entry is an ordinary declared type until then); the accessibility
pass over the editor's own surfaces (22 — the floor over plugin-authored
content is the plugin author's to meet, the contract documents it);
the fizzbuzz binary's UI mode, which consumes the whole editor (24); any
marketplace or dynamic loading — plugins and their UI link in at build
time (vision scope); and no new canvas gesture or interaction path — the
canvas behaves exactly as before, and a plugin's UI issues only the
existing edit operations, not new ones.

## Definition of done

- A plugin declares node-type UI through the one `inventory` registration
  path — a node type's UI naming its entry asset — with no second
  registration mechanism, and core names nothing specific to any plugin,
  node, or type anywhere in it. (REQ-8, REQ-9, REQ-39, REQ-42, REQ-73)
- A plugin declares type-value UI per data type through the same path — a
  Rust serialisation function and an entry asset for a display component;
  the serialiser is called generically by type reference where values of
  that type cross the bridge, and the default (no declaration) is story
  17's contentless crossing unchanged; core gains no knowledge of a custom
  type's payload beyond its registered metadata. (REQ-34, REQ-32, REQ-33,
  REQ-73)
- The HTTP server serves each linked plugin's declared assets under a
  per-plugin path, and the node-type listing and type facts carry the
  entry-asset facts per 11's framing — additive, computed from the
  declarations server-side, so the browser hardcodes no plugin, type, or
  path. A plugin declaring no UI adds no facts. (REQ-4, REQ-45, REQ-73)
- A node of a type with declared UI renders the plugin's component once
  its bundle loads, the component receiving the node's definition slice
  and the live display state — the run state beside the status marks and
  per-port latest values, custom types in the form their serialiser
  produces — and owning the body between title bar and ports; the shell —
  ports and title bar — remains editor-rendered, and wiring, unhooking,
  moving, selecting, deleting, panning, and zooming behave on it exactly
  as on any default node. Until the bundle arrives, and on every failure —
  a missing or unloadable asset, an unknown contract version, or a
  component that fails after loading, such as one that throws while
  rendering the live state — the node renders by the default class, the
  failure reported naming the type; the editor stays usable, never blank.
  (REQ-39, REQ-34, REQ-38, REQ-47)
- Editing issued from inside a plugin's UI rides the existing operation
  messages — label and parameter edits — landing server-side and pushing
  to every connection like any other edit; an edit issued while a run is
  on is rejected by 16's lock identically, and the run state the contract
  hands the component lets the plugin's own controls sit inert while a run
  is on, like every field the sidebar and the default class render. A
  plugin's UI defines no second editing path and the definition gains no
  field it did not already carry. (REQ-2, REQ-25, REQ-41)
- A custom type declared with a serialiser displays as the serialiser's
  output wherever its values display: in the per-port readouts of story
  17 on default nodes, and available to any node UI through the same
  display state; a custom type with no serialiser displays no invented
  content, exactly as 17 shipped. The serialisation is display-only —
  nothing the browser sends back deserialises, because no message carries
  a stream value in. (REQ-34, REQ-14, REQ-33)
- The component contract is versioned and the version rides the listing
  facts; a bundle naming an unknown contract version, or one that fails —
  to load, or after it, such as a component that throws while rendering —
  is reported through 18's surfaces naming the type, and the failed
  bundle's attachment point falls back on its own — a node-type UI to the
  default class, a type-value UI to 17's contentless readout — never the
  one taking the other down. The contract itself — what a component
  receives, what it may issue, its version, and the floor its author must
  meet — is documented for plugin authors, as the declaration macros'
  rustdoc is today, so a bundle can be built against it and the version
  names something checkable. Bundles load lazily — on the first node of
  the type or first display of the type's values — and a bundle that
  has not arrived delays nothing else: the rest of the canvas stays live.
  (REQ-71, REQ-45)
- Every open tab agrees: a custom node and a rendered custom-type value
  look the same after a reload and in a second tab, loaded from the same
  served assets against the same server-held definition. (REQ-2, REQ-45)
- Focused tests cover: the registration path for both attachment points;
  the entry-asset facts on the listings for a plugin with UI and their
  absence without; asset serving under the per-plugin path, including a
  missing asset answered as an error, not a crash; the serialiser invoked
  generically when its type's values cross the bridge and the contentless
  default when none is declared; the contract-version check rejecting an
  unknown version, and a component that fails after loading — one that
  throws while rendering the live state — falling back and reporting (the
  browser-side checks among these run in the UI test runner story 20's
  precedent adds to DEVELOPMENT.md's check set); and a plugin-issued
  parameter edit applying through the ordinary operation path, including
  its rejection while a run is on. The workspace builds and passes
  `cargo test` and `cargo clippy` cleanly, with the UI build part of the
  check set DEVELOPMENT.md documents.
- The visible proof, run per DEVELOPMENT.md: open the editor example —
  the shapes plugin, which it already links, gains the custom-UI node type
  and the type-value UI this proof needs — and see, with the palette
  populated but no custom node dropped and no custom value shown, that the
  browser has requested none of the plugin's bundles. Drop the custom
  node — it renders the plugin's own design, not the default class. The
  plugin also ships a default-rendered node emitting its custom type; wire
  its output into the custom node and watch the plugin's rendering of that
  value appear, and see the same value displayed at the emitting port's
  readout by the plugin's type-value component; edit a parameter through
  the plugin's own control and see it land in the sidebar field, in a
  second tab, and after a reload. Rename the node in the sidebar and
  see the plugin's rendering take the new label. Drag, wire, and delete
  the custom node and confirm every gesture behaves as it does on a
  default one. And once, deliberately: link the example against a bundle
  naming a contract version the editor does not speak — the node still
  renders by the default class and the report names the type.

## Comments

- 2026-09-16 — Scope seams: the transport seam and its assets-not-messages
  rule are 11's, settled there for this story to build on; the default node
  class this story replaces per type is 12 and 14's; the display state —
  statuses and per-port values — is 17's, and the contentless default this
  story gives content is exactly 17's shipped behaviour; the operation
  messages plugin edits ride are 13 and 14's; the report surface for a
  failed bundle is 18's; palette presentation is untouched — a custom-UI
  node's palette row stays 19's, since finding a type and rendering it are
  different moments. Groups (23) get whatever their collapsed nodes need
  when they exist; the a11y pass (22) covers the editor's own surfaces;
  24 consumes everything. (REQ-74)
- 2026-09-16 — UX calls settled here so plugin authors and later stories
  inherit them rather than relitigate: the shell — ports and title bar —
  stays editor-rendered on custom nodes — the gestures must not change
  because a node looks different, and the canvas library needs real ports
  to wire to; the sidebar stays generic rather than growing a
  plugin-authorable second surface — one consumer today does not justify
  a second UI contract; the serialisation is one-directional — nothing
  carries a stream value from the browser back, so a deserialiser would
  be a mechanism with no user; bundles load lazily so a plugin-rich
  palette stays fast; and there is no settings or toggle around custom
  UI — a plugin declares UI, the editor renders it, and the default
  rendering is the fallback, not an option to choose.
- 2026-09-16 — Why the mechanism is two attachment points rather than one
  "custom UI" grab-bag: a node's look and a type's value rendering answer
  different questions and attach at different times — node UI loads when
  the node is on the canvas, type-value UI when the type's value displays
  anywhere, including on nodes whose UI is default. Fusing them would make
  a plugin ship node UI to fix a value display, or declare node UI it
  does not have; keeping them separate lets the common case — a custom
  type that only wants to be seen — cost one component and one
  serialiser.
- 2026-09-16 — REQ-32 and REQ-34 name ser/de, and this story ships the
  serialiser only — a deliberate reading of REQ-34's "(where possible)",
  argued here: deserialisation answers a question nothing asks yet — no
  message carries a custom-typed value from browser to server, because
  parameters are plain scalars and values flow on wires — and the "de"
  half of the plugin's own data handling (REQ-32) stays inside the
  plugin's UI, which owns it, should its design ever want one. If a later
  story creates a path for browser-supplied custom values, its deserialiser
  rides this same registration — the mechanism leaves room without
  building an unused half today.
- 2026-09-16 — Why assets over messages, restated from this end: a bundle
  is code, the protocol is data; packing code through the websocket would
  make the graph channel carry programs, and every viewer of it would
  need to trust what arrives mid-session. Serving the binary's own linked
  declarations keeps the trust boundary where the vision put it — build
  time — and keeps the websocket for the definition and events it already
  carries.
- 2026-09-16 — Review calls, recorded so they hold: the shell is named
  exactly — ports and title bar, the move gesture's home and 19's icon's
  host — so the plugin's ownership is the body between, not whatever the
  first implementer guesses; the run state rides the contract from
  version 1, because a fact added later is a version bump that strands
  every bundle built before it; a failure after load — a component
  throwing while rendering — sits in the failure model beside the
  load-time ones, unhandled being the silent blank this story forbids;
  the two bundles fail independently, each attachment point falling back
  on its own; and the contract is itself a deliverable, documented for
  plugin authors, or the version names nothing checkable. The proof's
  readout step observes at the emitting port, per 17, on a
  default-rendered node of the plugin's — a default utility node emits
  core scalars, never the plugin's type — and the demo rides the shapes
  plugin, which the example already links and which already declares the
  custom type and nodes emitting it.
