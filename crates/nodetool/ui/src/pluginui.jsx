// The plugin UI contract, version 1 — the whole surface a plugin's bundle
// is built against. A plugin declares UI the way it declares everything
// else, on the Rust side: a node type's `ui` arm naming the bundle that
// renders that node type's body, and a data type's `ui` arm naming the
// serialiser its values cross through and the bundle that renders that
// form. The server carries both declarations to this page as flat facts on
// the listings — `ui: { entry, contract }` per type — and this module
// loads what they name. No fact, no custom UI; the browser hardcodes no
// plugin, type, or asset path.
//
// The contract a component receives, as props:
//
// - `h` — React's createElement: the bundle is a plain ES module, so it
//   builds its elements with the page's React rather than importing any.
// - `label` — the node's current label, the stored override else the
//   type's default.
// - `parameters` — the node's stored parameters, the stored values as they
//   are.
// - `connectedInputs` — the names of the inputs a wire reaches.
// - `status` — the node's derived run status, one of the bridge's names
//   (`running`, `completed`, `failed`, `stopped`), absent while untouched.
// - `marks` — the messages of every problem that names the node, compile's
//   and a failed run's alike. Rendering them is the floor the bundle's
//   author must meet, with the rest of the accessibility floor over their
//   own content: a plugin's UI is build-time linked code, never
//   sanitised — a plugin's own malformed UI is its declaration to fix.
// - `portValues` — the per-port latest values of the run, custom types in
//   the form their serialiser produced.
// - `locked` — whether a run is on: the definition is held still, and the
//   bundle's own editing controls sit inert, exactly as the editor's do.
// - `setLabel(text)` and `setParameter(input, text)` — the two edits a
//   bundle may issue, the same operations the sidebar and the default
//   node's inline fields ride. An empty parameter text unsets. There is no
//   second editing path and no other operation; the lock the server holds
//   rejects whatever a stale bundle still tries, like any other edit.
//
// A type-value component receives `h` and `value` — the serialised form —
// and renders one value wherever values display.
//
// A bundle that fails — an unknown contract version, a missing or
// unloadable asset, a component that throws while rendering — is reported
// through the toast surface naming the type, and the attachment point
// falls back on its own: a node-type UI to the default node class, a
// type-value UI to the contentless readout. A bundle loads once per page,
// on the first node of its type on the canvas or the first display of the
// type's values; until it arrives nothing else waits on it.

import { createContext, Component } from 'react'

// The contract this page offers. A bundle names the version it was built
// against; the listing carries that name, so a bundle the page does not
// understand is known before it loads — never half-rendered.
export const UI_CONTRACT_VERSION = 1

// The page's own dynamic import, as a bundle loader. Vite leaves a
// variable-specifier import alone; the browser fetches the served asset
// when the first attachment point asks for it.
export const importBundle = (entry) => import(/* @vite-ignore */ entry)

// Load one declared bundle: the fact must name a contract this page
// speaks, and the module must export the component. Every rejection is
// worded to be reported as it is — the caller prefixes the type.
export function loadBundle(fact, importModule = importBundle) {
  if (!fact || fact.contract !== UI_CONTRACT_VERSION) {
    return Promise.reject(
      new Error(
        `the bundle names UI contract ${fact?.contract ?? 'none'}, this editor speaks ${UI_CONTRACT_VERSION}`,
      ),
    )
  }
  return importModule(fact.entry).then((module) => {
    if (typeof module?.default !== 'function') {
      throw new Error('the bundle exports no component')
    }
    return module.default
  })
}

// One page-wide table of bundle loads: a bundle loads once, however many
// nodes and ports name its asset, and a failed load is not re-attempted —
// the report names the type once, the attachment points fall back, and a
// reload of the page is the retry. `attempt` answers the load's promise
// for the fact's entry, starting it if this is the first ask.
export function createBundleStore(importModule = importBundle) {
  const attempts = new Map()
  return {
    attempt(fact) {
      let load = attempts.get(fact.entry)
      if (load === undefined) {
        load = loadBundle(fact, importModule)
        attempts.set(fact.entry, load)
      }
      return load
    },
  }
}

// The type references whose node UI the canvas needs: the nodes on it
// whose type declares a bundle. A palette full of plugin types costs
// nothing — only a node on the canvas asks.
export function bodyTypeRefs(graph, types) {
  const refs = new Set()
  if (!graph || !types) return refs
  const declared = new Set(types.filter((type) => type.ui).map((type) => type.type_ref))
  for (const node of graph.nodes) {
    if (declared.has(node.type_ref)) refs.add(node.type_ref)
  }
  return refs
}

// The type references whose value UI the canvas needs: the ports a latest
// value now sits at, declared as exactly one type that itself declares a
// value bundle. A union port names no single bundle, a scalar no bundle at
// all — their values show as the plain text they always crossed as.
export function valueTypeRefs(graph, types, dataTypes, values) {
  const refs = new Set()
  if (!graph || !types || !values) return refs
  const byRef = new Map(types.map((type) => [type.type_ref, type]))
  for (const node of graph.nodes) {
    const type = byRef.get(node.type_ref)
    if (!type) continue
    for (const port of type.outputs) {
      if (values[`${node.uuid}/${port.name}`] === undefined) continue
      const ref = port.type_refs.length === 1 ? port.type_refs[0] : null
      if (ref && dataTypes?.[ref]?.ui) refs.add(ref)
    }
  }
  return refs
}

// The contract the editor hands a plugin component through, held here so
// the default node class and the port readouts read the loaded bundles and
// the report surface from one place. Null while the editor assembles.
export const PluginUiContext = createContext(null)

// The failure fence a loaded bundle renders inside: a component that
// throws — while rendering the live state, say — is caught here, reported
// by its caller, and renders nothing further; the attachment point falls
// back, never taking its node or port down.
export class UiBoundary extends Component {
  state = { failed: false }

  static getDerivedStateFromError() {
    return { failed: true }
  }

  componentDidCatch(error) {
    this.props.onFail(error)
  }

  render() {
    return this.state.failed ? null : this.props.children
  }
}
