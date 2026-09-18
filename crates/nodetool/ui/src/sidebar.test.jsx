/** @vitest-environment jsdom */
// The multi-selection's sidebar: the header naming the selection by its
// count, the common fields only — shared value, or the mixed marker —
// and the commit fanning out to the existing per-node parameter edit,
// one operation per node, an empty commit unsetting on every one. The
// fan-out is the story's seam: what arrives at the protocol is the same
// operation a single node's field sends.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { EditContext } from './fields.jsx'
import { SelectionSidebar } from './sidebar.jsx'

afterEach(cleanup)

const types = new Map([
  [
    'alpha/add',
    {
      type_ref: 'alpha/add',
      inputs: [
        { name: 'augend', type_refs: ['f64'] },
        { name: 'addend', type_refs: ['f64'] },
      ],
      outputs: [],
    },
  ],
])
const baseScalars = { f64: true }

function sidebar(nodes, edit, edges = []) {
  return render(
    <EditContext.Provider value={edit}>
      <SelectionSidebar nodes={nodes} types={types} baseScalars={baseScalars} edges={edges} />
    </EditContext.Provider>,
  )
}

describe('SelectionSidebar', () => {
  it('names the selection by its count and shows the common fields', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u3', type_ref: 'alpha/add', parameters: { addend: 7 } },
    ]
    sidebar(nodes, vi.fn())
    expect(screen.getByText('3 nodes selected')).toBeTruthy()
    expect(screen.getByLabelText('addend').value).toBe('7')
    expect(screen.getByLabelText('augend').value).toBe('')
  })

  it('a shared value shows in the field; differing values show the mixed marker', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 9 } },
    ]
    sidebar(nodes, vi.fn())
    expect(screen.getByLabelText('addend').value).toBe('')
    expect(screen.getByLabelText('addend').placeholder).toBe('mixed')
  })

  it('a commit lands on every selected node as the per-node parameter edit', () => {
    const edit = vi.fn()
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 7 } },
    ]
    sidebar(nodes, edit)
    fireEvent.change(screen.getByLabelText('addend'), { target: { value: '9' } })
    fireEvent.keyDown(screen.getByLabelText('addend'), { key: 'Enter' })
    expect(edit).toHaveBeenCalledTimes(2)
    expect(edit).toHaveBeenNthCalledWith(2, 'set_parameter', {
      uuid: 'u2',
      input: 'addend',
      value: '9',
    })
  })

  it('an empty commit unsets on every node, from the mixed state too', () => {
    const edit = vi.fn()
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 9 } },
    ]
    sidebar(nodes, edit)
    fireEvent.keyDown(screen.getByLabelText('addend'), { key: 'Enter' })
    expect(edit).toHaveBeenCalledTimes(2)
    expect(edit).toHaveBeenNthCalledWith(1, 'set_parameter', { uuid: 'u1', input: 'addend' })
    expect(edit).toHaveBeenNthCalledWith(2, 'set_parameter', { uuid: 'u2', input: 'addend' })
  })

  it('clicking into a mixed field and away without typing commits nothing', () => {
    const edit = vi.fn()
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 9 } },
    ]
    sidebar(nodes, edit)
    fireEvent.focus(screen.getByLabelText('addend'))
    fireEvent.blur(screen.getByLabelText('addend'))
    expect(edit).not.toHaveBeenCalled()
  })

  it('where nothing is common the sidebar says so plainly', () => {
    sidebar(
      [
        { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
        { uuid: 'u2', type_ref: 'gone/missing', parameters: {} },
      ],
      vi.fn(),
    )
    expect(screen.getByText('2 nodes selected')).toBeTruthy()
    expect(screen.getByText('no shared parameters')).toBeTruthy()
    expect(screen.queryByText('Label')).toBeNull()
  })
})
