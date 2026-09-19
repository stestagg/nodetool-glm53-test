// Groups: the document's named subgraphs, composed client-side from what
// the definition already carries — no new message, no second path. A group
// instance renders as one collapsed node carrying the group's exposed
// ports, its type reference resolved against the document's groups before
// the node-type listing's; its inside reports through the run's ordinary
// events, whose inner nodes sit under compiled identities derived from the
// chain of group instances enclosing them plus the node's own uuid — the
// derivation mirrored here, so which group instance and inner node an
// event names is recoverable without the server saying anything new.
//
// The collapsed node's status mark aggregates its inside: error when an
// inner node errors, completed when every inner node is completed, stopped
// when the run's closure left started inner nodes stopped, running only
// while inner nodes are running, and nothing for a group that never ran.
// Inner value pulses stay inside — collapsed means collapsed — except at
// the boundary: an inner emission from the node an exposed output binds
// animates the instance's own port and the wires that leave it, exactly
// the value flow the flat run carries across.
//
// The packaging gestures' client-side decisions are the same kind of
// composition — the name suggestion the dialog opens with and the group
// instances unpack acts on are read off the definition's groups, the
// browser assembling nothing of the format the server derives.

const MASK = (1n << 128n) - 1n

function uuidBits(uuid) {
  return BigInt(`0x${uuid.replaceAll('-', '')}`)
}

function bitsUuid(bits) {
  const hex = bits.toString(16).padStart(32, '0')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

// The compiled identity of a node inside a group, the mirror of
// nodetool::compile's inner_identity: each enclosing instance — innermost
// first — is XORed in and the accumulator rotated one bit, so chains that
// differ as paths derive identities that differ in turn.
export function innerIdentity(chain, node) {
  let id = uuidBits(node)
  for (const instance of chain) {
    const mixed = id ^ uuidBits(instance)
    id = ((mixed << 1n) & MASK) | (mixed >> 127n)
  }
  return bitsUuid(id)
}

// The type a group instance renders by: the group's exposed ports as its
// own, no icon, the group's name as the default label — the same shape the
// node-type listing's entries carry, so the default node class draws it.
export function groupType(node, group) {
  return {
    type_ref: node.type_ref,
    label: group.name,
    icon: null,
    inputs: group.inputs ?? [],
    outputs: group.outputs ?? [],
  }
}

// The first available name in the Group, Group 2, ... sequence — Enter
// alone packages, because the suggestion is one the server will take.
export function groupSuggestion(groups) {
  const taken = new Set((groups ?? []).map((group) => group.name))
  for (let n = 1; ; n += 1) {
    const name = n === 1 ? 'Group' : `Group ${n}`
    if (!taken.has(name)) return name
  }
}

// The group instances among `selected`: the nodes whose type reference a
// document group defines — unpack's operation runs once per one of them,
// the ordinary nodes in the selection left as they are.
export function groupInstances(selected, groups) {
  const names = new Set((groups ?? []).map((group) => group.name))
  return selected.filter((node) => names.has(node.type_ref)).map((node) => node.uuid)
}

// The document's group names, as the background menu's submenu lists
// them: alphabetical, the palette rows' deterministic order, so every tab
// and every reload shows the same menu.
export function groupNames(groups) {
  return (groups ?? [])
    .map((group) => group.name)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
}

// Everything the canvas and the event path need about a document's groups,
// or null when the document carries none — the flat case, no mapping work.
// `emissions` maps the inner node-and-port an exposed output binds to the
// instance port that shows it; `inner` holds every inner identity so an
// inside pulse stays inside; `owner` maps an inner identity to the
// top-level instance whose collapsed node holds it, for the failed run's
// mark; `inside` lists an instance's inner identities, for the status
// aggregate.
export function groupFacts(graph) {
  const groups = graph?.groups ?? []
  if (groups.length === 0) return null
  const byName = new Map(groups.map((group) => [group.name, group]))
  const facts = {
    types: byName,
    emissions: new Map(),
    inner: new Set(),
    owner: new Map(),
    inside: new Map(),
  }
  // One instance's whole inside: every plain inner node beneath it,
  // however deeply nested — the statuses its collapsed mark aggregates.
  // A nested instance is no node in the flat graph: it is recursed into,
  // never listed, so the aggregate reads real inner nodes only and a
  // completed run completes the group.
  const collect = (group, instanceUuid, chain, inside) => {
    for (const node of group.nodes ?? []) {
      const nested = byName.get(node.type_ref)
      if (nested !== undefined) {
        collect(nested, instanceUuid, [...chain, node.uuid], inside)
        continue
      }
      const identity = innerIdentity(chain, node.uuid)
      facts.inner.add(identity)
      facts.owner.set(identity, instanceUuid)
      inside.push(identity)
    }
  }
  // One exposed output, followed through the nested instances its binding
  // may cross, to the plain node that actually emits.
  const bind = (group, chain, exposedName, bound, boundPort) => {
    const target = (group.nodes ?? []).find((node) => node.uuid === bound)
    if (target === undefined) return
    const nested = byName.get(target.type_ref)
    if (nested === undefined) {
      facts.emissions.set(`${innerIdentity(chain, target.uuid)}/${boundPort}`, {
        instance: chain[0],
        port: exposedName,
      })
      return
    }
    const binding = (nested.outputs ?? []).find((port) => port.name === boundPort)
    if (binding !== undefined) {
      bind(nested, [...chain, target.uuid], exposedName, binding.node, binding.port)
    }
  }
  for (const node of graph?.nodes ?? []) {
    const group = byName.get(node.type_ref)
    if (group === undefined) continue
    const inside = []
    collect(group, node.uuid, [node.uuid], inside)
    facts.inside.set(node.uuid, inside)
    for (const port of group.outputs ?? []) {
      bind(group, [node.uuid], port.name, port.node, port.port)
    }
  }
  return facts
}

// The run display's latest values, re-keyed for the canvas: a boundary
// emission's text moves to the instance port that shows it, and an inside
// pulse's text is dropped — collapsed means collapsed.
export function canvasValues(values, facts) {
  if (!facts) return values
  const canvas = {}
  for (const [key, text] of Object.entries(values ?? {})) {
    const boundary = facts.emissions.get(key)
    if (boundary !== undefined) {
      canvas[`${boundary.instance}/${boundary.port}`] = text
    } else if (!facts.inner.has(key.slice(0, key.indexOf('/')))) {
      canvas[key] = text
    }
  }
  return canvas
}

// The collapsed node's status mark: the aggregate of its inside. Error
// when an inner node errored, stopped when the run's closure stopped some,
// running while any runs, completed when every inner node completed — and
// nothing when none ever started.
export function instanceStatus(statuses, innerUuids) {
  const states = innerUuids.map((uuid) => statuses?.[uuid])
  if (states.some((status) => status === 'failed')) return 'failed'
  if (states.some((status) => status === 'stopped')) return 'stopped'
  if (states.some((status) => status === 'running')) return 'running'
  if (innerUuids.length > 0 && states.every((status) => status === 'completed')) {
    return 'completed'
  }
  return undefined
}
