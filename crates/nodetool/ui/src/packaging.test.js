// The packaging gestures' client-side decisions and the canvas shape a
// packaged definition renders: the name suggestion the dialog opens
// with, the group instances unpack acts on, and the collapsed instance
// with the crossing wires landing on its exposed ports — the last read
// off the same toNodes/toEdges mappings every other canvas shape rides.
import { describe, expect, it } from 'vitest'
import { groupInstances, groupSuggestion } from './packaging.js'
import { toEdges, toNodes } from './view.js'

describe('groupSuggestion', () => {
  it('offers Group when nothing is taken', () => {
    expect(groupSuggestion([])).toBe('Group')
    expect(groupSuggestion(undefined)).toBe('Group')
  })

  it('walks the numbered names past the taken ones', () => {
    expect(groupSuggestion([{ name: 'Group' }])).toBe('Group 2')
    expect(groupSuggestion([{ name: 'Group' }, { name: 'Group 2' }])).toBe('Group 3')
  })

  it('unrelated names leave the plain suggestion available', () => {
    expect(groupSuggestion([{ name: 'stage' }, { name: 'filter' }])).toBe('Group')
  })
})

describe('groupInstances', () => {
  const groups = [{ name: 'stage' }, { name: 'filter' }]

  it('picks the selected nodes whose type reference a document group defines', () => {
    const selected = [
      { uuid: 'u1', type_ref: 'alpha/add' },
      { uuid: 'u2', type_ref: 'stage' },
      { uuid: 'u3', type_ref: 'filter' },
    ]
    expect(groupInstances(selected, groups)).toEqual(['u2', 'u3'])
  })

  it('a selection without instances, or a document without groups, unpacks nothing', () => {
    expect(groupInstances([{ uuid: 'u1', type_ref: 'alpha/add' }], groups)).toEqual([])
    expect(groupInstances([{ uuid: 'u2', type_ref: 'stage' }], [])).toEqual([])
  })
})

// The definition a package leaves: one instance typed by the group, the
// outside wires re-seated on its exposed ports, an untyped node's
// placeholder beside it. The canvas draws the collapsed node by the same
// default class as any type, its ports the group's.
describe('the packaged definition on the canvas', () => {
  const definition = {
    nodes: [
      {
        uuid: 'u1',
        type_ref: 'stage',
        metadata: { position: { x: 50, y: 0 } },
      },
      { uuid: 'u9', type_ref: 'gone/missing', metadata: {} },
    ],
    edges: [
      { from: 'u0', from_port: 'value', to: 'u1', to_port: 'b' },
      { from: 'u1', from_port: 'sum', to: 'u9', to_port: 'in' },
    ],
    groups: [
      {
        name: 'stage',
        inputs: [{ name: 'b', type_refs: ['i32'], node: 'n1', port: 'b' }],
        outputs: [{ name: 'sum', type_refs: ['i32'], node: 'n2', port: 'sum' }],
        nodes: [],
        edges: [],
      },
    ],
  }
  const listing = {
    types: [{ type_ref: 'alpha/add', label: 'Add', inputs: [], outputs: [] }],
    dataTypes: { i32: { color: '#2d72d2', shape: 'square' } },
  }

  it('renders the instance as a collapsed node carrying the group’s ports, at its recorded seat', () => {
    const [node] = toNodes(definition, listing)
    expect(node).toMatchObject({
      id: 'u1',
      position: { x: 50, y: 0 },
      type: 'type',
    })
    expect(node.data.type).toMatchObject({
      type_ref: 'stage',
      label: 'stage',
      inputs: [expect.objectContaining({ name: 'b' })],
      outputs: [expect.objectContaining({ name: 'sum' })],
    })
  })

  it('the wires that touched the selection land on the instance’s ports', () => {
    const edges = toEdges(definition, new Set(), listing)
    expect(edges.map((edge) => [edge.source, edge.sourceHandle, edge.target, edge.targetHandle]))
      .toEqual([
        ['u0', 'value', 'u1', 'b'],
        ['u1', 'sum', 'u9', 'in'],
      ])
    // The wire colour reads the exposed port's declared type.
    expect(edges[1].style.stroke).toBe('#2d72d2')
  })
})
