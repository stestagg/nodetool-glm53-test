// The definition-to-nodes mapping is pure: data in, node shapes out, no DOM
// and no React Flow. These are the DoD behaviours the visible proof cannot
// produce — no linked binary carries an unknown type reference, and nothing
// yet writes a placeless node.
import { describe, expect, it } from 'vitest'
import { toNodes } from './editor.jsx'

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
})
