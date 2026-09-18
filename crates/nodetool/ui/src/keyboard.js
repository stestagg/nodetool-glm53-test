// The keyboard's way around the editor: the decision each key press
// makes, shaped as the same operation its pointer gesture sends — a
// palette row's activation the create a drop sends, a focused port's
// activation the wire drag sends, Delete on a connected input the
// drag-off sends, a nudge the drag stop's move sends. The shell turns
// each decision into the operation; pan and zoom stay pointer-only,
// focus being the keyboard's way around the canvas.

// The flow position the view's centre names: where a keyboard-created
// node lands — deterministic given the view, visible by construction.
export function viewportCenter(viewport, size) {
  return {
    x: Math.round((size.width / 2 - viewport.x) / viewport.zoom),
    y: Math.round((size.height / 2 - viewport.y) / viewport.zoom),
  }
}

// The moves one canvas change batch commits: the rests — a drag stop's
// final change and each keyboard nudge carry the position the node
// landed on, while a drag's travel frames report `dragging` — as the
// move operation, one per node that landed somewhere new.
export function finalMoves(changes) {
  return changes
    .filter(
      (change) =>
        change.type === 'position' && change.dragging !== true && change.position != null,
    )
    .map((change) => ({ uuid: change.id, position: change.position }))
}

// The wire operation's fields for landing a wire on `candidate` when it
// runs from `from` — either end may hold the running end, the operation
// always naming source then target — or null when the candidate cannot
// take it: its own port, or its own side.
export function wireLanding(from, candidate) {
  if (from.type === candidate.type) return null
  if (from.node === candidate.node && from.port === candidate.port) return null
  const [out, into] = from.type === 'source' ? [from, candidate] : [candidate, from]
  return { from: out.node, from_port: out.port, to: into.node, to_port: into.port }
}

// What a key press on a focused port does, or null: Delete on a connected
// input unhooks it — the drag-off's operation; Enter or Space starts a
// wire from the port, or lands the one in progress on it. The lock takes
// the port's keys away with every other edit; navigation and focus stay.
export function portKey(event, port, wire, editable) {
  if (!editable) return null
  if (event.key === 'Delete' || event.key === 'Backspace') {
    if (port.type === 'target' && port.wired) {
      return { kind: 'unhook', fields: { to: port.node, to_port: port.port } }
    }
    return null
  }
  if (event.key !== 'Enter' && event.key !== ' ') return null
  if (wire === null) return { kind: 'start' }
  return { kind: 'land', fields: wireLanding(wire, port) }
}

// The shift-activation on a focused node: the node it names, or null —
// the shift-click's twin, taken before the canvas's own selection keys
// see it. A plain activation stays the canvas's, the plain click's.
export function toggleKey(event) {
  if (!event.shiftKey || (event.key !== 'Enter' && event.key !== ' ')) return null
  return event.target.closest?.('.react-flow__node')?.getAttribute('data-id') ?? null
}

// The minimal pan that brings a screen-space rect inside the canvas's,
// or null when nothing needs moving: the view following focus onto the
// canvas, a port outside the reach of the pane brought back by the
// shortest shift.
export function panIntoView(viewport, canvasRect, rect) {
  if (canvasRect === undefined || canvasRect === null) return null
  const dx =
    rect.left < canvasRect.left
      ? rect.left - canvasRect.left
      : Math.max(0, rect.right - canvasRect.right)
  const dy =
    rect.top < canvasRect.top
      ? rect.top - canvasRect.top
      : Math.max(0, rect.bottom - canvasRect.bottom)
  if (dx === 0 && dy === 0) return null
  return { x: viewport.x - dx, y: viewport.y - dy, zoom: viewport.zoom }
}
