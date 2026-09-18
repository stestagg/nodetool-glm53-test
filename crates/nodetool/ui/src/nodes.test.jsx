/** @vitest-environment jsdom */
// The port's key dispatch: a decided act turned into the operation at
// the seam. Enter with no wire running starts one, Enter with one
// running lands it — the wire standing down before the operation is
// sent, an invalid landing standing down with nothing sent — Delete on a
// connected input unhooks, and the lock takes the keys away with every
// other edit. portKey's pure tests pin the decision; these pin the
// dispatch, rendered the way sidebar.test.jsx and palette.test.jsx
// render theirs.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ReactFlowProvider } from '@xyflow/react'
import { EditContext, LockContext, WireContext } from './fields.jsx'
import { TypeNode } from './nodes.jsx'

afterEach(cleanup)

// The identity type's opposite ports share a name — the self-wire case
// the pointer drag lands and the keyboard must land too.
const type = {
  type_ref: 'beta/identity',
  label: 'Identity',
  inputs: [{ name: 'value', type_refs: ['i32'] }],
  outputs: [{ name: 'value', type_refs: ['i32'] }],
}

function node({ edit = vi.fn(), setWire = vi.fn(), wire = null, locked = false } = {}) {
  render(
    <EditContext.Provider value={edit}>
      <LockContext.Provider value={locked}>
        <WireContext.Provider value={{ wire, setWire }}>
          <ReactFlowProvider>
            <TypeNode
              selected={false}
              data={{
                node: { uuid: 'u5', type_ref: 'beta/identity', parameters: {} },
                type,
                wiredInputs: ['value'],
                scalarInputs: [],
                marks: [],
                portValues: {},
                dataTypes: { i32: { color: '#2d72d2', shape: 'square' } },
              }}
            />
          </ReactFlowProvider>
        </WireContext.Provider>
      </LockContext.Provider>
    </EditContext.Provider>,
  )
  return {
    edit,
    setWire,
    input: screen.getByLabelText('Identity value input'),
    output: screen.getByLabelText('Identity value output'),
  }
}

describe('the port key dispatch', () => {
  it('Enter with no wire running starts one from the port', () => {
    const { edit, setWire, input } = node()
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(setWire).toHaveBeenCalledWith({ node: 'u5', port: 'value', type: 'target' })
    expect(edit).not.toHaveBeenCalled()
  })

  it('Enter with a wire running lands it: the wire stands down, the operation is sent', () => {
    const wire = { node: 'u1', port: 'sum', type: 'source' }
    const { edit, setWire, input } = node({ wire })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(setWire).toHaveBeenCalledWith(null)
    expect(edit).toHaveBeenCalledWith('wire', {
      from: 'u1',
      from_port: 'sum',
      to: 'u5',
      to_port: 'value',
    })
    expect(setWire.mock.invocationCallOrder[0]).toBeLessThan(edit.mock.invocationCallOrder[0])
  })

  it('the same node’s opposite ports share a name and still land — the self-wire', () => {
    const wire = { node: 'u5', port: 'value', type: 'target' }
    const { edit, output } = node({ wire })
    fireEvent.keyDown(output, { key: 'Enter' })
    expect(edit).toHaveBeenCalledWith('wire', {
      from: 'u5',
      from_port: 'value',
      to: 'u5',
      to_port: 'value',
    })
  })

  it('a landing on the running port itself stands the wire down with nothing sent', () => {
    const wire = { node: 'u5', port: 'value', type: 'source' }
    const { edit, setWire, output } = node({ wire })
    fireEvent.keyDown(output, { key: 'Enter' })
    expect(setWire).toHaveBeenCalledWith(null)
    expect(edit).not.toHaveBeenCalled()
  })

  it('Delete on a connected input unhooks it', () => {
    const { edit, setWire, input } = node()
    fireEvent.keyDown(input, { key: 'Delete' })
    expect(edit).toHaveBeenCalledWith('unhook', { to: 'u5', to_port: 'value' })
    expect(setWire).not.toHaveBeenCalled()
  })

  it('the lock takes the keys away: nothing starts, lands, or unhooks', () => {
    const wire = { node: 'u1', port: 'sum', type: 'source' }
    const { edit, setWire, input } = node({ wire, locked: true })
    fireEvent.keyDown(input, { key: 'Enter' })
    fireEvent.keyDown(input, { key: 'Delete' })
    expect(setWire).not.toHaveBeenCalled()
    expect(edit).not.toHaveBeenCalled()
  })
})
