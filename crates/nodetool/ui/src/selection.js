// The multi-selection model, view-side. The selection is the editor's own
// state — the server neither knows nor cares that it existed — and
// everything it changes fans out to the catalogue's per-node operations:
// what the sidebar may edit in common, and what a commit through the
// selection sends. A field is common when every selected node's type
// declares the input, scalar-possible on every one, and connected on none
// — the only edit that can land on every node at once; a plugin's node
// joins for free by declaring scalar ports, and a placeholder, its ports
// unknown, contributes nothing.

import { commitParameter, scalarPossible, scalarText } from './fields.jsx'

// The plain ordering every view agrees on, the palette's own.
const byName = (a, b) => (a < b ? -1 : a > b ? 1 : 0)

// The fields the selection holds in common, in name order. Each carries
// the value the field shows: the shared text when every selected node
// holds the same one, blank and `mixed` when they differ — blank alone
// must keep meaning unset-everywhere, so the mixed state has its own
// marker.
export function commonFields(nodes, types, baseScalars, edges) {
  const wired = new Set(edges.map((edge) => `${edge.to}/${edge.to_port}`))
  let names = null
  for (const node of nodes) {
    const declared = new Set(
      (types.get(node.type_ref)?.inputs ?? [])
        .filter((port) => scalarPossible(port, baseScalars))
        .map((port) => port.name),
    )
    names = names === null ? declared : new Set([...names].filter((name) => declared.has(name)))
  }
  return [...(names ?? [])]
    .filter((name) => nodes.every((node) => !wired.has(`${node.uuid}/${name}`)))
    .sort(byName)
    .map((name) => {
      const texts = nodes.map((node) => scalarText(node.parameters?.[name]))
      const mixed = !texts.every((text) => text === texts[0])
      return { name, mixed, text: mixed ? '' : texts[0] }
    })
}

// Commit one common field across the selection: the existing parameter
// edit, once per node — an empty value unsetting on every one, from the
// mixed state as from an unset one. The server answers each as it
// answers any edit; a refusal for one node leaves the others as they
// landed.
export function commitCommon(edit, nodes, input, value) {
  for (const node of nodes) commitParameter(edit, node.uuid, input, value)
}
