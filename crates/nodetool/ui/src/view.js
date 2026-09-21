// The definition-to-canvas mappings and the gesture decisions, pure: data
// in, node and wire shapes out, no DOM and no React Flow. The editor shell
// renders what these produce and reads its gestures through them, so the
// visible proof exercises them by hand once and the tests pin them.

import { SelectionMode, applyNodeChanges } from '@xyflow/react'
import { groupFacts, groupType, instanceStatus } from './groups.js'
import { scalarPossible } from './fields.jsx'
import { portAppearance } from './types.js'

// The fixed grid a node without a recorded position lands on, ordered by
// uuid: a deterministic, view-local fallback every view and reload agrees
// on, with nothing written into the definition.
const FALLBACK_PITCH = { x: 180, y: 140 }

function recordedPosition(node) {
  const position = node.metadata?.position
  if (typeof position?.x === 'number' && typeof position?.y === 'number') {
    return { x: position.x, y: position.y }
  }
  return undefined
}

// `facts` — the document's group facts — arrives composed where the
// caller already holds them (the editor keeps one per definition
// arrival); absent, it is derived here.
export function toNodes(graph, listing, marks, statuses, values, facts = groupFacts(graph)) {
  // A group instance's type reference resolves against the document's
  // groups before the listing's, so a group never reads as unknown.
  const byRef = new Map((listing?.types ?? []).map((type) => [type.type_ref, type]))
  const dataTypes = listing?.dataTypes ?? {}
  const placeless = graph.nodes
    .filter((node) => recordedPosition(node) === undefined)
    .sort((a, b) => (a.uuid < b.uuid ? -1 : 1))
    .map((node, index) => [
      node.uuid,
      {
        x: 80 + (index % 5) * FALLBACK_PITCH.x,
        y: 80 + Math.floor(index / 5) * FALLBACK_PITCH.y,
      },
    ])
  const fallback = new Map(placeless)
  const wiredInputs = new Map()
  for (const edge of graph.edges) {
    const ports = wiredInputs.get(edge.to) ?? []
    ports.push(edge.to_port)
    wiredInputs.set(edge.to, ports)
  }
  const at = (uuid) => marks?.get(uuid) ?? []
  const status = (uuid) => statuses?.[uuid]
  const value = (uuid, port) => values?.[`${uuid}/${port}`]
  return graph.nodes.map((node) => {
    const position = recordedPosition(node) ?? fallback.get(node.uuid)
    const group = facts?.types.get(node.type_ref)
    const type = group !== undefined ? groupType(node, group) : byRef.get(node.type_ref)
    if (type === undefined) {
      return {
        id: node.uuid,
        position,
        type: 'placeholder',
        data: { label: node.label ?? node.type_ref, marks: at(node.uuid), status: status(node.uuid) },
        selectable: true,
      }
    }
    const portValues = {}
    for (const port of type.outputs) {
      const text = value(node.uuid, port.name)
      if (text !== undefined) portValues[port.name] = text
    }
    // A collapsed group's status is its inside's aggregate; every other
    // node's status is its own.
    const derived =
      group !== undefined
        ? instanceStatus(statuses, facts.inside.get(node.uuid) ?? [])
        : status(node.uuid)
    return {
      id: node.uuid,
      position,
      type: 'type',
      data: {
        node,
        type,
        wiredInputs: wiredInputs.get(node.uuid) ?? [],
        scalarInputs: type.inputs
          .filter((port) => scalarPossible(port, listing?.baseScalars))
          .map((port) => port.name),
        marks: at(node.uuid),
        status: derived,
        portValues,
        dataTypes,
      },
    }
  })
}

// What the editor calls a node: its label, else the label of the type it
// instantiates, else the type reference. Never its uuid — that is the
// address wires and marks travel on, not a name anybody reads.
export function nodeName(node, type) {
  return node.label ?? type?.label ?? node.type_ref
}

// The definition's edges as canvas wires. Not selectable: drag-off is the
// one way a wire comes off, so there is no second, selected-then-deleted
// path. A wire renders in the colour of the source port's declared type —
// the neutral where no single type informs the port; a wire in `pulsing`
// animates, the pulse a value travels riding that colour as a dash flow.
export function toEdges(graph, pulsing, listing, facts = groupFacts(graph)) {
  const byRef = new Map((listing?.types ?? []).map((type) => [type.type_ref, type]))
  const dataTypes = listing?.dataTypes ?? {}
  const refOf = new Map(graph.nodes.map((node) => [node.uuid, node.type_ref]))
  const typeOf = (ref) => facts?.types.get(ref) ?? byRef.get(ref)
  const nameOf = new Map(
    graph.nodes.map((node) => [node.uuid, nodeName(node, typeOf(node.type_ref))]),
  )
  return graph.edges.map((edge) => {
    const id = `${edge.from}/${edge.from_port}->${edge.to}/${edge.to_port}`
    const port = typeOf(refOf.get(edge.from))?.outputs.find((output) => output.name === edge.from_port)
    return {
      id,
      // React Flow announces a wire by its two node ids; the canvas
      // speaks the names the user gave it instead.
      ariaLabel: `wire from ${nameOf.get(edge.from)} ${edge.from_port} to ${nameOf.get(edge.to)} ${edge.to_port}`,
      source: edge.from,
      sourceHandle: edge.from_port,
      target: edge.to,
      targetHandle: edge.to_port,
      selectable: false,
      animated: pulsing?.has(id) === true,
      type: edge.from === edge.to ? 'selfloop' : undefined,
      style: { stroke: portAppearance(port, dataTypes).color },
    }
  })
}

// The wires one emission animates: every downstream wire of the emitting
// port — a fan-out pulses each path the value actually takes.
export function emittedTargets(edges, node, port) {
  return edges
    .filter((edge) => edge.from === node && edge.from_port === port)
    .map((edge) => `${edge.from}/${edge.from_port}->${edge.to}/${edge.to_port}`)
}

// What a wire drag's ending means when no connection landed: a release on
// a port — any port, even one the wire cannot land on, the wire's own
// included — is the wire gesture and cancels quietly; a connected input's
// end released anywhere that is not a port unhooks it. React Flow reports
// whatever port sits under the release, valid or not, as `toHandle`.
// Answers the unhook operation's fields, or null.
export function offPortUnhook(edges, state) {
  const from = state.fromHandle
  if (from === null || from.type !== 'target' || state.toHandle != null) return null
  const wired = edges.some(
    (edge) => edge.to === from.nodeId && edge.to_port === from.id,
  )
  return wired ? { to: from.nodeId, to_port: from.id } : null
}

// Whether a keyboard event's target sits in a text field: the keys keep
// their editing meaning there, so the canvas's own — delete among them —
// wait for focus to return.
export function inTextField(target) {
  return target?.closest?.('input, textarea, select, [contenteditable]') != null
}

// Which nodes the delete key removes: a multi-selection takes every
// selected node, placeholders included — uuid-addressed operations need
// no type knowledge — while a selection of one is story 12's delete, a
// typed node spared nothing and a lone placeholder spared.
export function deleteSelection(nodes) {
  const selected = nodes.filter((node) => node.selected)
  if (selected.length === 1 && selected[0].type === 'placeholder') return []
  return selected.map((node) => node.id)
}

// Which nodes a Delete/Backspace keystroke removes, or none — the whole
// guard chain in one place. The gesture lives on the canvas in focus: a
// text field keeps the keys as text editing, and so does focus anywhere
// else in the chrome, while no focused element at all reads as the
// canvas's — the canvas takes no focus of its own. The lock takes the
// gesture away with every other edit, and a held key repeats nothing.
export function deleteKeys(event, editable, nodes, canvas) {
  if (!editable || event.repeat) return []
  if (event.key !== 'Delete' && event.key !== 'Backspace') return []
  if (inTextField(event.target)) return []
  if (!canvas?.contains(event.target) && event.target !== canvas?.ownerDocument?.body) {
    return []
  }
  return deleteSelection(nodes)
}

// A placeholder takes part in the uuid-addressed gestures exactly when it
// is selected inside a multi-selection and editing is unlocked: story 12
// keeps a lone placeholder inert, a placeholder outside the selection
// never travels on another pair's account, and the lock holds it as it
// holds every node. This is the draggable flag the canvas drags read,
// set from the selection wherever selection changes.
export function withDraggablePlaceholders(nodes, editable) {
  const multi = nodes.filter((node) => node.selected).length > 1
  return nodes.map((node) =>
    node.type === 'placeholder'
      ? { ...node, draggable: multi && editable && node.selected }
      : node,
  )
}

// One node toggled in or out of the selection, the others untouched —
// the shift-click's model, and the shift-activation key's.
export function toggledSelection(nodes, uuid) {
  return nodes.map((node) =>
    node.id === uuid ? { ...node, selected: !node.selected } : node,
  )
}

// What a definition push's rebuilt node keeps from the one the canvas
// holds: the selection, a drag's in-flight position, and the measurement
// the canvas already took. The measurement is the quiet one — a rebuilt
// node without it is measured afresh, and until the measuring lands the
// canvas holds the node invisible, which drops keyboard focus off the
// canvas mid-run.
export function carriedNode(node, held) {
  if (held === undefined) return node
  return {
    ...node,
    selected: held.selected ?? false,
    position: held.dragging ? held.position : node.position,
    measured: held.measured,
  }
}

// The background's two drags differ by Shift alone, and so does their
// catch: a plain drag pans, a shift-drag draws the marquee, and the
// marquee takes every node the rectangle touches — inside or
// intersecting, the catch a sweep across a clump expects. The props the
// canvas reads for all of it.
export function backgroundDrag(shift) {
  return {
    selectionOnDrag: shift,
    panOnDrag: !shift,
    selectionMode: SelectionMode.Partial,
  }
}

// The node changes the canvas applies, save removal: a remove change
// would drop nodes the server still holds, with no operation sent and no
// refusal path — deletion is the shell's own gesture, fanned to the
// delete operation and applied by the definition push that answers it.
export function viewNodeChanges(changes, nodes) {
  return applyNodeChanges(
    changes.filter((change) => change.type !== 'remove'),
    nodes,
  )
}

// Whether the held graph carries unsaved changes — the one condition the
// discard guard asks under, silence there being the one way to lose work.
export function hasUnsavedChanges(file) {
  return file?.dirty === true
}

// Whether a save asks for its target: saving elsewhere names a path, and
// so does the first save of an untitled graph; any other save writes the
// file being edited without asking.
export function saveAsksForPath(file, elsewhere) {
  return elsewhere || file?.path == null
}

// The chrome's run group: what each of its three controls can do in the
// state the server holds. Start needs an idle editor and something to
// run, an empty definition leaving it disabled — the editor session
// itself is the interactive mode, so there is none to switch on. Stop
// needs a run on. Reset needs a run to clear: one on, or one finished
// whose outcome and canvas still stand. All three reach the server, so a
// connection that cannot deliver them is the same as no controls.
export function runControls(run, graph, connected = true) {
  const running = run?.running === true
  return {
    start: connected && !running && (graph?.nodes.length ?? 0) > 0,
    stop: connected && running,
    reset: connected && (running || (run?.outcome ?? null) !== null),
  }
}

// Where each problem lives, as the canvas draws it: one mark per node,
// carrying the messages of every problem that names it — the compile's
// attribution read as a structure, never parsed from messages — plus the
// failed run's error at the node whose failure ended it. `owner` maps an
// inner node's compiled identity to the group instance whose collapsed
// node holds it, so a failure inside a group is marked where it shows.
export function nodeMarks(problems, run, owner = null) {
  const marks = new Map()
  const add = (uuid, message) => {
    const list = marks.get(uuid) ?? []
    list.push(message)
    marks.set(uuid, list)
  }
  for (const problem of problems ?? []) {
    for (const uuid of problem.nodes ?? []) add(uuid, problem.message)
  }
  if (run?.outcome === 'failed' && run.node) {
    add(owner?.get(run.node) ?? run.node, run.error)
  }
  return marks
}

// The banner a connection state names, or none: the loss names itself and
// the reconnection the client is already making; the mismatch names itself
// and the reload that is the advice — never a self-dismissing toast. A
// connected or not-yet-lost editor has no banner.
export function bannerText(connection, mismatch) {
  if (connection === 'lost') return 'connection lost — trying to reconnect…'
  if (connection === 'incompatible') return `${mismatch} — reload the page`
  return null
}

// The status line's reading of the run state: the running state itself,
// or the last run's outcome, a failure naming what failed.
export function runStatusText(run) {
  if (run?.running === true) return 'running'
  if (run?.outcome === 'failed') return `run failed: ${run.error ?? 'unknown failure'}`
  if (run?.outcome) return `run ${run.outcome}`
  return ''
}
