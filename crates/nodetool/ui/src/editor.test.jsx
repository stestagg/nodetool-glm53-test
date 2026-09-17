// The definition-to-canvas mappings are pure: data in, node and wire
// shapes out, no DOM and no React Flow — as is the reading of a finished
// wire drag. These are the DoD behaviours the visible proof cannot
// produce — no linked binary carries an unknown type reference, and
// nothing yet writes a placeless node.
import { describe, expect, it } from 'vitest'
import { offPortUnhook, toEdges, toNodes } from './editor.jsx'

describe('toNodes', () => {
  it('renders an unknown type reference as the inert placeholder, labelled with the stored override else the type reference', () => {
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
      selectable: false,
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
    const types = [{ type_ref: 'alpha/add' }]
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
      { type_ref: 'alpha/add' },
      { type_ref: 'beta/identity' },
    ]
    const nodes = toNodes(graph, types)
    expect(nodes[0].data.wiredInputs).toEqual([])
    expect(nodes[1].data.wiredInputs).toEqual(['value', 'other'])
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
    const state = { fromHandle: { type: 'target', nodeId: 'u3', id: 'value' }, isValid: null }
    expect(offPortUnhook(edges, state)).toEqual({ to: 'u3', to_port: 'value' })
  })

  it('a landing is the wire gesture, not an unhook', () => {
    const state = { fromHandle: { type: 'target', nodeId: 'u3', id: 'value' }, isValid: true }
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
