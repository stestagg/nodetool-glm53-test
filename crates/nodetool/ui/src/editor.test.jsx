// The definition-to-canvas mappings are pure: data in, node and wire
// shapes out, no DOM and no React Flow — as is the reading of a finished
// wire drag, the save's ask-or-not decision, and the discard guard's
// condition. The visible proof exercises them by hand once; these tests
// pin them: the placeholder and grid shapes the editor's real data never
// produces (no linked binary carries an unknown type reference, nothing
// yet writes a placeless node), the scalar-input classification off the
// listing's base-scalar fact, the wire-drag partition, whose
// release-on-a-port ending must never unhook, the file chrome's two
// destructive-if-wrong decisions — when a save asks for its path, and
// when a replacement asks about discarding — the marks the compile's
// problems and the failed run place at their nodes, and the banner a
// lost connection raises.
import { describe, expect, it } from 'vitest'
import {
  bannerText,
  hasUnsavedChanges,
  nodeMarks,
  offPortUnhook,
  runControl,
  runStatusText,
  saveAsksForPath,
  toEdges,
  toNodes,
} from './editor.jsx'

describe('toNodes', () => {
  it('renders an unknown type reference as the placeholder, labelled with the stored override else the type reference, selectable for its label edit', () => {
    const nodes = toNodes(
      {
        edges: [],
        nodes: [
          { uuid: 'u1', type_ref: 'shapes/circle', label: 'The circle', metadata: {} },
          { uuid: 'u2', type_ref: 'text/uppercase', metadata: {} },
        ],
      },
      [],
    )
    expect(nodes[0]).toMatchObject({
      id: 'u1',
      type: 'placeholder',
      draggable: false,
      selectable: true,
      data: { label: 'The circle' },
    })
    expect(nodes[1]).toMatchObject({
      id: 'u2',
      type: 'placeholder',
      data: { label: 'text/uppercase' },
    })
  })

  it('lands placeless nodes on distinct grid slots a re-render agrees on', () => {
    const graph = {
      edges: [],
      nodes: [
        { uuid: 'b6a529e2-4b58-4a3d-9f01-2f6f4a1d9c33', type_ref: 'alpha/add', metadata: {} },
        { uuid: '7c9e6679-7425-40de-944b-e07fc1f90ae7', type_ref: 'alpha/add', metadata: {} },
      ],
    }
    const types = [{ type_ref: 'alpha/add', inputs: [] }]
    const nodes = toNodes(graph, types)
    expect(nodes[0].position).not.toEqual(nodes[1].position)
    expect(toNodes(graph, types)).toEqual(nodes)
  })

  it('names the inputs a wire reaches, so the node can show the connected state', () => {
    const graph = {
      edges: [
        { from: 'u1', from_port: 'sum', to: 'u2', to_port: 'value' },
        { from: 'u1', from_port: 'sum', to: 'u2', to_port: 'other' },
      ],
      nodes: [
        { uuid: 'u1', type_ref: 'alpha/add', metadata: {} },
        { uuid: 'u2', type_ref: 'beta/identity', metadata: {} },
      ],
    }
    const types = [
      { type_ref: 'alpha/add', inputs: [] },
      { type_ref: 'beta/identity', inputs: [] },
    ]
    const nodes = toNodes(graph, types)
    expect(nodes[0].data.wiredInputs).toEqual([])
    expect(nodes[1].data.wiredInputs).toEqual(['value', 'other'])
  })

  it('names the scalar-possible inputs off the base-scalar fact, unions included', () => {
    const graph = {
      edges: [],
      nodes: [
        { uuid: 'u1', type_ref: 'alpha/mix', metadata: {} },
        { uuid: 'u2', type_ref: 'beta/custom', metadata: {} },
      ],
    }
    const types = [
      {
        type_ref: 'alpha/mix',
        inputs: [
          { name: 'text', type_refs: ['String'] },
          { name: 'ratio', type_refs: ['alpha/ratio'] },
          { name: 'either', type_refs: ['alpha/ratio', 'f64'] },
        ],
      },
      { type_ref: 'beta/custom', inputs: [{ name: 'only', type_refs: ['alpha/ratio'] }] },
    ]
    const baseScalars = { String: true, 'alpha/ratio': false, f64: true }
    const nodes = toNodes(graph, types, baseScalars)
    expect(nodes[0].data.scalarInputs).toEqual(['text', 'either'])
    expect(nodes[1].data.scalarInputs).toEqual([])
  })
})

describe('toEdges', () => {
  it('draws the definition\'s edges as wires between the named ports', () => {
    const graph = {
      edges: [{ from: 'u1', from_port: 'sum', to: 'u2', to_port: 'value' }],
      nodes: [],
    }
    expect(toEdges(graph)).toEqual([
      {
        id: 'u1/sum->u2/value',
        source: 'u1',
        sourceHandle: 'sum',
        target: 'u2',
        targetHandle: 'value',
        selectable: false,
      },
    ])
  })

  it('marks a wire a node runs back into itself as the loop edge', () => {
    const graph = {
      edges: [{ from: 'u1', from_port: 'sum', to: 'u1', to_port: 'value' }],
      nodes: [],
    }
    expect(toEdges(graph)).toEqual([
      {
        id: 'u1/sum->u1/value',
        source: 'u1',
        sourceHandle: 'sum',
        target: 'u1',
        targetHandle: 'value',
        selectable: false,
        type: 'selfloop',
      },
    ])
  })

  it('tells two wires apart by the edges they carry', () => {
    const graph = {
      edges: [
        { from: 'u1', from_port: 'sum', to: 'u2', to_port: 'value' },
        { from: 'u1', from_port: 'sum', to: 'u3', to_port: 'value' },
      ],
      nodes: [],
    }
    const [first, second] = toEdges(graph)
    expect(first.id).not.toEqual(second.id)
  })
})

describe('offPortUnhook', () => {
  const edges = [
    { from: 'u1', from_port: 'sum', to: 'u2', to_port: 'value' },
    { from: 'u1', from_port: 'sum', to: 'u3', to_port: 'value' },
  ]

  it('unhooks the wire a connected input\'s end carries when released off-port', () => {
    const state = {
      fromHandle: { type: 'target', nodeId: 'u3', id: 'value' },
      isValid: null,
      toHandle: null,
    }
    expect(offPortUnhook(edges, state)).toEqual({ to: 'u3', to_port: 'value' })
  })

  it('a landing is the wire gesture, not an unhook', () => {
    const state = {
      fromHandle: { type: 'target', nodeId: 'u3', id: 'value' },
      isValid: true,
      toHandle: { type: 'source', nodeId: 'u1', id: 'sum' },
    }
    expect(offPortUnhook(edges, state)).toBeNull()
  })

  it('a release on a port the wire cannot land on is the wire gesture, not an unhook', () => {
    const state = {
      fromHandle: { type: 'target', nodeId: 'u3', id: 'value' },
      isValid: false,
      toHandle: { type: 'target', nodeId: 'u2', id: 'value' },
    }
    expect(offPortUnhook(edges, state)).toBeNull()
  })

  it('a release back on the wire\'s own port keeps the wire', () => {
    const state = {
      fromHandle: { type: 'target', nodeId: 'u3', id: 'value' },
      isValid: null,
      toHandle: { type: 'target', nodeId: 'u3', id: 'value' },
    }
    expect(offPortUnhook(edges, state)).toBeNull()
  })

  it('a drag from an output cancels quietly', () => {
    const state = { fromHandle: { type: 'source', nodeId: 'u1', id: 'sum' }, isValid: null }
    expect(offPortUnhook(edges, state)).toBeNull()
  })

  it('an input with no wire changes nothing', () => {
    const state = { fromHandle: { type: 'target', nodeId: 'u9', id: 'value' }, isValid: null }
    expect(offPortUnhook(edges, state)).toBeNull()
  })
})

describe('saveAsksForPath', () => {
  it('saving elsewhere always asks', () => {
    expect(saveAsksForPath({ path: 'graphs/a.yml', dirty: false }, true)).toBe(true)
  })

  it('the first save of an untitled graph asks', () => {
    expect(saveAsksForPath(null, false)).toBe(true)
    expect(saveAsksForPath({ path: null, dirty: false }, false)).toBe(true)
  })

  it('a plain save of a named file writes it without asking', () => {
    expect(saveAsksForPath({ path: 'graphs/a.yml', dirty: true }, false)).toBe(false)
  })
})

describe('hasUnsavedChanges', () => {
  it('a dirty graph has them; a clean or not-yet-known one has not', () => {
    expect(hasUnsavedChanges({ path: 'graphs/a.yml', dirty: true })).toBe(true)
    expect(hasUnsavedChanges({ path: null, dirty: false })).toBe(false)
    expect(hasUnsavedChanges(null)).toBe(false)
  })
})

describe('runControl', () => {
  it('flips Start↔Stop with the run state, the states mutually exclusive', () => {
    expect(runControl({ running: false, outcome: null }, { nodes: [{}] })).toEqual({
      running: false,
      label: 'Start',
      enabled: true,
    })
    expect(runControl({ running: true }, { nodes: [{}] })).toEqual({
      running: true,
      label: 'Stop',
      enabled: true,
    })
  })

  it('an empty definition has nothing to run: Start disabled, Stop unaffected', () => {
    expect(runControl({ running: false, outcome: null }, { nodes: [] }).enabled).toBe(false)
    expect(runControl(null, null).enabled).toBe(false)
    expect(runControl({ running: true }, { nodes: [] }).enabled).toBe(true)
  })

  it('a run state not yet resynced reads as an idle start', () => {
    expect(runControl(null, { nodes: [{}] })).toEqual({
      running: false,
      label: 'Start',
      enabled: true,
    })
  })

  it('both acts reach the server: no connection, no control', () => {
    expect(runControl({ running: true }, { nodes: [{}] }, false)).toEqual({
      running: true,
      label: 'Stop',
      enabled: false,
    })
    expect(runControl({ running: false, outcome: null }, { nodes: [{}] }, false).enabled).toBe(
      false,
    )
  })
})

describe('nodeMarks', () => {
  it('places each problem\'s message at the nodes it names, one mark carrying all of them', () => {
    const marks = nodeMarks(
      [
        { message: 'no port `nope`', nodes: ['u1'] },
        { message: 'no exact match', nodes: ['u1', 'u2'] },
      ],
      null,
    )
    expect(marks.get('u1')).toEqual(['no port `nope`', 'no exact match'])
    expect(marks.get('u2')).toEqual(['no exact match'])
    expect(marks.size).toBe(2)
  })

  it('the failed run\'s error sits at the node whose failure ended it', () => {
    const marks = nodeMarks(
      [{ message: 'a compile problem', nodes: ['u1'] }],
      { outcome: 'failed', node: 'u2', error: 'node Failer (u2): the failer ran' },
    )
    expect(marks.get('u1')).toEqual(['a compile problem'])
    expect(marks.get('u2')).toEqual(['node Failer (u2): the failer ran'])
  })

  it('a clean graph and a run that did not fail carry nothing', () => {
    expect(nodeMarks([], { outcome: 'completed' }).size).toBe(0)
    expect(nodeMarks([], { outcome: 'failed', node: null }).size).toBe(0)
    expect(nodeMarks(undefined, null).size).toBe(0)
  })

  it('a new run resets the canvas: the failed mark is gone', () => {
    const marks = nodeMarks(
      [],
      { running: true },
    )
    expect(marks.size).toBe(0)
  })
})

describe('toNodes marks', () => {
  it('carries the node\'s marks in its data, placeholder included', () => {
    const graph = {
      edges: [],
      nodes: [
        { uuid: 'u1', type_ref: 'alpha/add', metadata: {} },
        { uuid: 'u2', type_ref: 'shapes/circle', metadata: {} },
      ],
    }
    const marks = new Map([
      ['u1', ['a problem']],
      ['u2', ['the unknown-type error']],
    ])
    const nodes = toNodes(graph, [{ type_ref: 'alpha/add', inputs: [] }], {}, marks)
    expect(nodes[0].data.marks).toEqual(['a problem'])
    expect(nodes[1].data.marks).toEqual(['the unknown-type error'])
  })

  it('a node nothing names carries no marks', () => {
    const nodes = toNodes(
      { edges: [], nodes: [{ uuid: 'u1', type_ref: 'alpha/add', metadata: {} }] },
      [{ type_ref: 'alpha/add', inputs: [] }],
    )
    expect(nodes[0].data.marks).toEqual([])
  })
})

describe('bannerText', () => {
  it('names the loss and the trying the client is already doing', () => {
    expect(bannerText('lost', null)).toBe('connection lost — trying to reconnect…')
  })

  it('names the version mismatch, reloading the page as the advice', () => {
    expect(bannerText('incompatible', 'the server speaks protocol 2')).toBe(
      'the server speaks protocol 2 — reload the page',
    )
  })

  it('a connected or still-trying editor has no banner', () => {
    expect(bannerText('open', null)).toBeNull()
    expect(bannerText('connecting', null)).toBeNull()
  })
})

describe('runStatusText', () => {
  it('a running run says so; an outcome tells how it ended', () => {
    expect(runStatusText({ running: true })).toBe('running')
    expect(runStatusText({ running: false, outcome: 'completed' })).toBe('run completed')
    expect(runStatusText({ running: false, outcome: 'stopped' })).toBe('run stopped')
  })

  it('a failed run names what failed', () => {
    expect(
      runStatusText({ running: false, outcome: 'failed', error: 'node Failer: the failer ran' }),
    ).toBe('run failed: node Failer: the failer ran')
  })

  it('an idle run with no outcome says nothing', () => {
    expect(runStatusText({ running: false, outcome: null })).toBe('')
    expect(runStatusText(null)).toBe('')
  })
})
