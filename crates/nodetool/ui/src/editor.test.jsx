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
import { NEUTRAL } from './types.js'
import {
  bannerText,
  emittedTargets,
  hasUnsavedChanges,
  nodeMarks,
  offPortUnhook,
  paletteGroups,
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
      { types: [] },
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
    const listing = { types: [{ type_ref: 'alpha/add', inputs: [], outputs: [] }] }
    const nodes = toNodes(graph, listing)
    expect(nodes[0].position).not.toEqual(nodes[1].position)
    expect(toNodes(graph, listing)).toEqual(nodes)
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
      { type_ref: 'alpha/add', inputs: [], outputs: [] },
      { type_ref: 'beta/identity', inputs: [], outputs: [] },
    ]
    const nodes = toNodes(graph, { types })
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
        outputs: [],
        inputs: [
          { name: 'text', type_refs: ['String'] },
          { name: 'ratio', type_refs: ['alpha/ratio'] },
          { name: 'either', type_refs: ['alpha/ratio', 'f64'] },
        ],
      },
      { type_ref: 'beta/custom', inputs: [{ name: 'only', type_refs: ['alpha/ratio'] }], outputs: [] },
    ]
    const listing = { types, baseScalars: { String: true, 'alpha/ratio': false, f64: true } }
    const nodes = toNodes(graph, listing)
    expect(nodes[0].data.scalarInputs).toEqual(['text', 'either'])
    expect(nodes[1].data.scalarInputs).toEqual([])
  })

  it('carries the listing’s colour-and-shape fact in the node data, for the ports to read', () => {
    const graph = { edges: [], nodes: [{ uuid: 'u1', type_ref: 'alpha/add', metadata: {} }] }
    const types = [{ type_ref: 'alpha/add', inputs: [], outputs: [] }]
    const dataTypes = { i32: { color: '#2d72d2', shape: 'square' } }
    const nodes = toNodes(graph, { types, dataTypes })
    expect(nodes[0].data.dataTypes).toEqual(dataTypes)
  })
})

describe('paletteGroups', () => {
  const types = [
    { type_ref: 'text/split', label: 'Split', plugin: 'text', sub_group: null },
    { type_ref: 'text/check', label: 'Check', plugin: 'text', sub_group: null },
    { type_ref: 'shapes/sphere', label: 'Sphere', plugin: 'shapes', sub_group: '3d' },
    { type_ref: 'shapes/polygon', label: 'Polygon', plugin: 'shapes', sub_group: '2d' },
    { type_ref: 'shapes/circle', label: 'Circle', plugin: 'shapes', sub_group: '2d' },
    { type_ref: 'shapes/rectangle', label: 'Rectangle', plugin: 'shapes', sub_group: '2d' },
    { type_ref: 'alpha/mix', label: 'Mix', plugin: 'alpha', sub_group: null },
    { type_ref: 'alpha/add', label: 'Add', plugin: 'alpha', sub_group: 'math' },
    { type_ref: 'alpha/concat', label: 'Concat', plugin: 'alpha', sub_group: 'text' },
  ]

  it('groups by plugin and sub-group, plugins and sub-groups alphabetical', () => {
    const groups = paletteGroups(types)
    expect(groups.map((group) => group.plugin)).toEqual(['alpha', 'shapes', 'text'])
    expect(groups[0].sections.map((section) => section.subGroup)).toEqual([null, 'math', 'text'])
    expect(groups[1].sections.map((section) => section.subGroup)).toEqual(['2d', '3d'])
  })

  it('types order alphabetically within their group', () => {
    const [, shapes] = paletteGroups(types)
    const [twoD, threeD] = shapes.sections
    expect(twoD.types.map((type) => type.label)).toEqual(['Circle', 'Polygon', 'Rectangle'])
    expect(threeD.types.map((type) => type.label)).toEqual(['Sphere'])
  })

  it('types without a sub-group sit directly under the plugin header, first', () => {
    const [alpha] = paletteGroups(types)
    expect(alpha.sections[0].subGroup).toBeNull()
    expect(alpha.sections[0].types.map((type) => type.label)).toEqual(['Mix'])
  })

  it('a flat plugin has no sub-sections', () => {
    const [, , text] = paletteGroups(types)
    expect(text.sections).toEqual([
      {
        subGroup: null,
        types: [
          expect.objectContaining({ label: 'Check' }),
          expect.objectContaining({ label: 'Split' }),
        ],
      },
    ])
  })

  it('every reload and tab reads the same sections', () => {
    expect(paletteGroups(types)).toEqual(paletteGroups([...types].reverse()))
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
        animated: false,
        type: undefined,
        style: { stroke: NEUTRAL.color },
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
        animated: false,
        type: 'selfloop',
        style: { stroke: NEUTRAL.color },
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

describe('toEdges colours', () => {
  const types = [
    {
      type_ref: 'text/uppercase',
      inputs: [{ name: 'text', type_refs: ['String'] }],
      outputs: [{ name: 'text', type_refs: ['String'] }],
    },
    {
      type_ref: 'text/ticker',
      inputs: [{ name: 'count', type_refs: ['i64'] }],
      outputs: [{ name: 'value', type_refs: ['i64'] }],
    },
    {
      type_ref: 'shapes/circle',
      inputs: [{ name: 'radius', type_refs: ['i32', 'f64'] }],
      outputs: [{ name: 'shape', type_refs: ['shapes/shape'] }],
    },
  ]
  const dataTypes = {
    String: { color: '#238551', shape: 'circle' },
    i64: { color: '#2d72d2', shape: 'square' },
    'shapes/shape': { color: '#8f99a8', shape: 'circle' },
  }
  const graph = {
    edges: [
      { from: 'u1', from_port: 'text', to: 'u2', to_port: 'text' },
      { from: 'u3', from_port: 'value', to: 'u4', to_port: 'count' },
      { from: 'u5', from_port: 'shape', to: 'u6', to_port: 'text' },
    ],
    nodes: [
      { uuid: 'u1', type_ref: 'text/uppercase', metadata: {} },
      { uuid: 'u2', type_ref: 'text/check', metadata: {} },
      { uuid: 'u3', type_ref: 'text/ticker', metadata: {} },
      { uuid: 'u4', type_ref: 'shapes/polygon', metadata: {} },
      { uuid: 'u5', type_ref: 'shapes/circle', metadata: {} },
      { uuid: 'u6', type_ref: 'text/uppercase', metadata: {} },
    ],
  }
  const listing = { types, dataTypes }

  it('a wire renders in the colour of the source port’s declared type', () => {
    const [string, integer] = toEdges(graph, undefined, listing)
    expect(string.style.stroke).toBe('#238551')
    expect(integer.style.stroke).toBe('#2d72d2')
  })

  it('a source port with no single declared type colours its wire the neutral', () => {
    const [, , union] = toEdges(graph, undefined, listing)
    expect(union.style.stroke).toBe(NEUTRAL.color)
  })

  it('a wire whose source node the listing does not know colours the neutral', () => {
    const unknown = {
      edges: [{ from: 'gone', from_port: 'out', to: 'u1', to_port: 'text' }],
      nodes: [{ uuid: 'gone', type_ref: 'gone/missing', metadata: {} }],
    }
    expect(toEdges(unknown, undefined, listing)[0].style.stroke).toBe(NEUTRAL.color)
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
    expect(nodeMarks([], { running: true }).size).toBe(0)
    expect(nodeMarks(undefined, null).size).toBe(0)
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
    const nodes = toNodes(
      graph,
      { types: [{ type_ref: 'alpha/add', inputs: [], outputs: [] }] },
      marks,
    )
    expect(nodes[0].data.marks).toEqual(['a problem'])
    expect(nodes[1].data.marks).toEqual(['the unknown-type error'])
  })

  it('a node nothing names carries no marks', () => {
    const nodes = toNodes(
      { edges: [], nodes: [{ uuid: 'u1', type_ref: 'alpha/add', metadata: {} }] },
      { types: [{ type_ref: 'alpha/add', inputs: [], outputs: [] }] },
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

describe('toNodes run display', () => {
  const type = {
    type_ref: 'delta/counter',
    inputs: [],
    outputs: [{ name: 'out', type_refs: ['i32'] }],
  }
  const graph = { edges: [], nodes: [{ uuid: 'u1', type_ref: 'delta/counter', metadata: {} }] }

  it('carries the derived status and the latest value per output port in its data', () => {
    const nodes = toNodes(graph, { types: [type] }, new Map(), { u1: 'running' }, { 'u1/out': '17' })
    expect(nodes[0].data.status).toBe('running')
    expect(nodes[0].data.portValues).toEqual({ out: '17' })
  })

  it('a port without a value and a node without a status carry neither', () => {
    const nodes = toNodes(graph, { types: [type] }, new Map(), {}, {})
    expect(nodes[0].data.status).toBeUndefined()
    expect(nodes[0].data.portValues).toEqual({})
  })

  it('a placeholder carries the status too, its ports unknown', () => {
    const unknown = { edges: [], nodes: [{ uuid: 'u2', type_ref: 'gone/missing', metadata: {} }] }
    const nodes = toNodes(unknown, { types: [type] }, new Map(), { u2: 'stopped' }, {})
    expect(nodes[0].data.status).toBe('stopped')
  })
})

describe('toEdges pulsing', () => {
  const graph = {
    edges: [{ from: 'a', from_port: 'out', to: 'b', to_port: 'in' }],
    nodes: [],
  }

  it('a wire in the pulse set animates — the dash flow riding its type colour', () => {
    const id = 'a/out->b/in'
    const traveling = toEdges(graph, new Set([id]))[0]
    expect(traveling.animated).toBe(true)
    expect(traveling.style.stroke).toBe(NEUTRAL.color)
    expect(toEdges(graph, new Set())[0].animated).toBe(false)
    expect(toEdges(graph)[0].animated).toBe(false)
  })
})

describe('emittedTargets', () => {
  const edges = [
    { from: 'src', from_port: 'out', to: 'x', to_port: 'a' },
    { from: 'src', from_port: 'out', to: 'y', to_port: 'value' },
    { from: 'other', from_port: 'out', to: 'z', to_port: 'in' },
  ]

  it('a fan-out pulses every downstream wire of the emitting port', () => {
    expect(emittedTargets(edges, 'src', 'out')).toEqual(['src/out->x/a', 'src/out->y/value'])
  })

  it('a port with no wires pulses nothing, and neither does another node\'s emission', () => {
    expect(emittedTargets(edges, 'src', 'spare')).toEqual([])
    expect(emittedTargets(edges, 'other', 'out')).toEqual(['other/out->z/in'])
  })
})
