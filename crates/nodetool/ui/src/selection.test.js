// The multi-selection's common-field rule, pinned on the real listing
// shapes: an input is common when every selected node's type declares it
// scalar-possible and no edge reaches it on any of them; the value reads
// uniform or mixed; a placeholder contributes nothing. The commit is the
// per-node parameter edit, once per node, an empty value unsetting on
// every one.
import { describe, expect, it, vi } from 'vitest'
import { commitCommon, commonFields } from './selection.js'

const baseScalars = { String: true, f64: true, 'alpha/ratio': false }
const types = new Map([
  [
    'alpha/add',
    {
      type_ref: 'alpha/add',
      inputs: [
        { name: 'augend', type_refs: ['f64'] },
        { name: 'addend', type_refs: ['f64'] },
        { name: 'text', type_refs: ['String'] },
        { name: 'ratio', type_refs: ['alpha/ratio'] },
      ],
      outputs: [],
    },
  ],
  [
    'alpha/mix',
    {
      type_ref: 'alpha/mix',
      inputs: [
        { name: 'text', type_refs: ['String'] },
        { name: 'ratio', type_refs: ['alpha/ratio', 'f64'] },
      ],
      outputs: [],
    },
  ],
  [
    'alpha/only',
    { type_ref: 'alpha/only', inputs: [{ name: 'spare', type_refs: ['f64'] }], outputs: [] },
  ],
])

function selection(nodes, edges = []) {
  return commonFields(nodes, types, baseScalars, edges)
}

function named(fields, name) {
  return fields.find((field) => field.name === name)
}

describe('commonFields', () => {
  it('shows the inputs every type declares scalar-possible, in name order', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: {} },
    ]
    expect(selection(nodes).map((field) => field.name)).toEqual(['addend', 'augend', 'text'])
  })

  it('intersects across different types: the name both declare scalar-possibly', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'alpha/mix', parameters: {} },
    ]
    expect(selection(nodes).map((field) => field.name)).toEqual(['text'])
  })

  it('a name only some of the types declare is not common', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'alpha/mix', parameters: {} },
    ]
    expect(named(selection(nodes), 'augend')).toBeUndefined()
  })

  it('a wire into the input on any selected node takes the field away', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: {} },
    ]
    const edges = [{ from: 'u3', from_port: 'out', to: 'u2', to_port: 'text' }]
    expect(selection(nodes, edges).map((field) => field.name)).toEqual(['addend', 'augend'])
  })

  it('a placeholder contributes nothing, so a selection holding one holds no fields', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'gone/missing', parameters: {} },
    ]
    expect(selection(nodes)).toEqual([])
  })

  it('the value reads as the shared text when every node holds the same one', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 7 } },
    ]
    expect(named(selection(nodes), 'addend')).toEqual({
      name: 'addend',
      mixed: false,
      text: '7',
    })
  })

  it('values that differ read mixed: blank with the marker, never one node value', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 9 } },
    ]
    expect(named(selection(nodes), 'addend')).toEqual({ name: 'addend', mixed: true, text: '' })
  })

  it('unset everywhere reads as blank without the marker — blank means unset-everywhere', () => {
    const mixed = selection([
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: {} },
    ])
    const unset = selection([
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: {} },
    ])
    expect(named(mixed, 'addend')).toEqual({ name: 'addend', mixed: true, text: '' })
    expect(named(unset, 'addend')).toEqual({ name: 'addend', mixed: false, text: '' })
  })

  it('the same name on different scalar types is still common: the stored values are what is shared', () => {
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { text: 'same' } },
      { uuid: 'u2', type_ref: 'alpha/mix', parameters: { text: 'same' } },
    ]
    expect(named(selection(nodes), 'text')).toEqual({ name: 'text', mixed: false, text: 'same' })
  })
})

describe('commitCommon', () => {
  it('sends the existing parameter edit once per node', () => {
    const edit = vi.fn()
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: {} },
      { uuid: 'u3', type_ref: 'alpha/add', parameters: {} },
    ]
    commitCommon(edit, nodes, 'addend', '9')
    expect(edit).toHaveBeenCalledTimes(3)
    expect(edit).toHaveBeenNthCalledWith(2, 'set_parameter', {
      uuid: 'u2',
      input: 'addend',
      value: '9',
    })
  })

  it('an empty commit unsets on every node — no value rides any message', () => {
    const edit = vi.fn()
    const nodes = [
      { uuid: 'u1', type_ref: 'alpha/add', parameters: { addend: 7 } },
      { uuid: 'u2', type_ref: 'alpha/add', parameters: { addend: 9 } },
    ]
    commitCommon(edit, nodes, 'addend', '')
    expect(edit).toHaveBeenCalledTimes(2)
    expect(edit).toHaveBeenNthCalledWith(1, 'set_parameter', { uuid: 'u1', input: 'addend' })
    expect(edit).toHaveBeenNthCalledWith(2, 'set_parameter', { uuid: 'u2', input: 'addend' })
  })
})
