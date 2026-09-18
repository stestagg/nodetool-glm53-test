// The keyboard's decisions, pure: the keyboard-created node's position,
// the moves a canvas change batch commits, the port keys that start,
// land, and unhook wires, the landings a candidate can take, Escape's
// quiet cancel over the selection and an in-progress wire, the
// shift-activation that toggles a node in the selection, and the
// view-follow that brings a focused port back. Each answer names the
// operation its pointer gesture sends; the shell turns the answer into
// the operation.
import { describe, expect, it } from 'vitest'
import {
  escapeCancel,
  finalMoves,
  panIntoView,
  portKey,
  toggleKey,
  viewportCenter,
  wireLanding,
} from './keyboard.js'

describe('viewportCenter', () => {
  it('the view centre is the flow position the keyboard creates at', () => {
    expect(viewportCenter({ x: 0, y: 0, zoom: 1 }, { width: 800, height: 600 })).toEqual({
      x: 400,
      y: 300,
    })
  })

  it('pan and zoom move the centre: the created node lands where the user is looking', () => {
    expect(viewportCenter({ x: -400, y: -100, zoom: 0.5 }, { width: 800, height: 600 })).toEqual({
      x: 1600,
      y: 800,
    })
  })

  it('the position is whole, a recorded metadata position like any other', () => {
    expect(
      viewportCenter({ x: 1, y: 1, zoom: 2 }, { width: 301, height: 201 }),
    ).toEqual({ x: 75, y: 50 })
  })
})

describe('finalMoves', () => {
  const rest = (id, x, y) => ({ id, type: 'position', position: { x, y } })
  const travel = (id, x, y) => ({ id, type: 'position', position: { x, y }, dragging: true })

  it('a drag stop and a keyboard nudge commit; a travel frame does not', () => {
    expect(finalMoves([travel('u1', 1, 1), rest('u1', 30, 40)])).toEqual([
      { uuid: 'u1', position: { x: 30, y: 40 } },
    ])
  })

  it('one move per node that rested, multi-selections included', () => {
    expect(finalMoves([rest('u1', 10, 10), rest('u2', 20, 20)])).toEqual([
      { uuid: 'u1', position: { x: 10, y: 10 } },
      { uuid: 'u2', position: { x: 20, y: 20 } },
    ])
  })

  it('selection and removal changes commit nothing', () => {
    expect(finalMoves([{ id: 'u1', type: 'select', selected: true }, { id: 'u1', type: 'remove' }]))
      .toEqual([])
  })
})

describe('wireLanding', () => {
  const out = { node: 'u1', port: 'sum', type: 'source' }
  const into = { node: 'u2', port: 'value', type: 'target' }

  it('either end may hold the running end; the operation names source then target', () => {
    expect(wireLanding(out, into)).toEqual({
      from: 'u1',
      from_port: 'sum',
      to: 'u2',
      to_port: 'value',
    })
    expect(wireLanding(into, out)).toEqual({
      from: 'u1',
      from_port: 'sum',
      to: 'u2',
      to_port: 'value',
    })
  })

  it('a port of its own side and the running port itself take nothing', () => {
    expect(wireLanding(out, { node: 'u3', port: 'other', type: 'source' })).toBeNull()
    expect(wireLanding(out, out)).toBeNull()
  })

  it('a node wiring back into itself is a landing like any other', () => {
    expect(
      wireLanding(out, { node: 'u1', port: 'value', type: 'target' }),
    ).toEqual({ from: 'u1', from_port: 'sum', to: 'u1', to_port: 'value' })
  })

  it('one node’s opposite ports may share a name — the self-wire a pointer drag lands', () => {
    const value = (type) => ({ node: 'u5', port: 'value', type })
    expect(wireLanding(value('source'), value('target'))).toEqual({
      from: 'u5',
      from_port: 'value',
      to: 'u5',
      to_port: 'value',
    })
    expect(wireLanding(value('target'), value('source'))).toEqual({
      from: 'u5',
      from_port: 'value',
      to: 'u5',
      to_port: 'value',
    })
  })
})

describe('portKey', () => {
  const into = (wired) => ({ node: 'u2', port: 'value', type: 'target', wired })
  const out = { node: 'u1', port: 'sum', type: 'source', wired: false }
  const press = (key) => ({ key })

  it('Enter or Space starts a wire from the port when none runs', () => {
    expect(portKey(press('Enter'), out, null, true)).toEqual({ kind: 'start' })
    expect(portKey(press(' '), into(true), null, true)).toEqual({ kind: 'start' })
  })

  it('with a wire running, the press lands it: the wire operation, replace included', () => {
    const running = { node: 'u1', port: 'sum', type: 'source' }
    expect(portKey(press('Enter'), into(false), running, true)).toEqual({
      kind: 'land',
      fields: { from: 'u1', from_port: 'sum', to: 'u2', to_port: 'value' },
    })
  })

  it('landing on the running port or its own side stands the wire down quietly', () => {
    const running = { node: 'u1', port: 'sum', type: 'source' }
    expect(portKey(press('Enter'), out, running, true)).toEqual({ kind: 'land', fields: null })
    expect(portKey(press(' '), out, running, true)).toEqual({ kind: 'land', fields: null })
  })

  it('Delete on a connected input unhooks it; on anything else it keeps the canvas meaning', () => {
    expect(portKey(press('Delete'), into(true), null, true)).toEqual({
      kind: 'unhook',
      fields: { to: 'u2', to_port: 'value' },
    })
    expect(portKey(press('Backspace'), into(true), null, true)).toEqual({
      kind: 'unhook',
      fields: { to: 'u2', to_port: 'value' },
    })
    expect(portKey(press('Delete'), into(false), null, true)).toBeNull()
    expect(portKey(press('Delete'), out, null, true)).toBeNull()
  })

  it('the lock takes the port keys away, navigation and focus staying', () => {
    expect(portKey(press('Enter'), out, null, false)).toBeNull()
    expect(portKey(press('Delete'), into(true), null, false)).toBeNull()
  })

  it('any other key is nothing', () => {
    expect(portKey(press('Tab'), out, null, true)).toBeNull()
  })
})

describe('panIntoView', () => {
  const viewport = { x: 0, y: 0, zoom: 1 }
  const canvas = { left: 0, top: 0, right: 800, bottom: 600 }

  it('a visible port pans nothing', () => {
    const rect = { left: 100, top: 100, right: 140, bottom: 140 }
    expect(panIntoView(viewport, canvas, rect)).toBeNull()
  })

  it('a port past an edge comes back by the shortest shift', () => {
    expect(
      panIntoView(viewport, canvas, { left: 820, top: 100, right: 860, bottom: 140 }),
    ).toEqual({ x: -60, y: 0, zoom: 1 })
    expect(
      panIntoView(viewport, canvas, { left: -50, top: -50, right: -10, bottom: -10 }),
    ).toEqual({ x: 50, y: 50, zoom: 1 })
  })

  it('the shift is screen-space: the viewport translation moves by the overhang', () => {
    const zoomed = { x: 40, y: 40, zoom: 0.5 }
    expect(
      panIntoView(zoomed, canvas, { left: 780, top: 100, right: 830, bottom: 140 }),
    ).toEqual({ x: 10, y: 40, zoom: 0.5 })
  })

  it('no canvas to measure against pans nothing', () => {
    expect(panIntoView(viewport, undefined, { left: 1, top: 1, right: 2, bottom: 2 })).toBeNull()
  })
})

describe('escapeCancel', () => {
  const drawn = (id, selected) => ({ id, selected })
  const nodes = [drawn('u1', true), drawn('u2', true), drawn('u3', false)]
  const inField = { target: { closest: () => ['input'] } }
  const press = (target = {}) => ({ key: 'Escape', target })

  it('Escape stands the wire down — whatever ran — and clears the selection', () => {
    expect(escapeCancel(press(), nodes)).toEqual({
      wire: null,
      nodes: [drawn('u1', false), drawn('u2', false), drawn('u3', false)],
    })
  })

  it('any other key is no cancel', () => {
    expect(escapeCancel({ key: 'Enter', target: {} }, nodes)).toBeNull()
  })

  it('a text field keeps the Escape it already had', () => {
    expect(escapeCancel(press(inField.target), nodes)).toBeNull()
  })
})

describe('toggleKey', () => {
  const inNode = (id) => ({
    target: {
      closest: (selector) =>
        selector === '.react-flow__node' ? { getAttribute: () => id } : null,
    },
  })
  const inField = { target: { closest: () => ['input'] } }

  it('Shift with Enter or Space names the focused node, the shift-click’s twin', () => {
    expect(toggleKey({ shiftKey: true, key: 'Enter', ...inNode('u2') })).toBe('u2')
    expect(toggleKey({ shiftKey: true, key: ' ', ...inNode('u2') })).toBe('u2')
  })

  it('a plain activation stays the canvas’s own, and another key is nothing', () => {
    expect(toggleKey({ shiftKey: false, key: 'Enter', ...inNode('u2') })).toBeNull()
    expect(toggleKey({ shiftKey: true, key: 'a', ...inNode('u2') })).toBeNull()
  })

  it('a target outside a node names nothing', () => {
    expect(toggleKey({ shiftKey: true, key: 'Enter', target: {} })).toBeNull()
  })

  it('a text field keeps the keys', () => {
    expect(toggleKey({ shiftKey: true, key: 'Enter', ...inField })).toBeNull()
  })
})
