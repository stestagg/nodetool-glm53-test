// The multi-selection model, view-side. The selection is the editor's own
// state — the server neither knows nor cares that it existed — and
// everything it changes fans out to the catalogue's per-node operations.
// A field is common when every selected node's type declares the input,
// scalar-possible on every one, and connected on none — the only edit
// that can land on every node at once; a plugin's node joins for free by
// declaring scalar ports, and a placeholder, its ports unknown,
// contributes nothing.

import { commitParameter, scalarPossible, scalarText } from './fields.jsx'
import { byName } from './types.js'

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
