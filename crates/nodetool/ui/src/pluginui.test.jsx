/** @vitest-environment jsdom */
// The plugin UI contract, on the real components. A bundle built against a
// contract the page does not speak is rejected before it loads, naming the
// contract versions; a bundle that fails to load, and a component that
// throws while rendering the live state, are reported and fall back — a
// node-type UI to the default class, a type-value UI to the contentless
// readout — never taking their node or port down. The contract a healthy
// component receives is pinned here too: the definition slice, the live
// display state, and the two edit helpers riding the ordinary operations.
// Which bundles the canvas asks for, and which it never does, is the lazy
// loading rule — a palette asks for nothing.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import {
  PluginUiContext,
  bodyTypeRefs,
  createBundleStore,
  loadBundle,
  valueTypeRefs,
} from './pluginui.jsx'
import { EditContext } from './fields.jsx'
import { TypeNode } from './nodes.jsx'
import { ReactFlowProvider } from '@xyflow/react'
import { createElement } from 'react'

afterEach(cleanup)

const FACT = { entry: '/plugins/epsilon/widget-node.js', contract: 1 }

describe('loadBundle', () => {
  it('resolves a bundle naming the contract this editor speaks, to its component', async () => {
    const body = () => null
    const component = await loadBundle(FACT, () => Promise.resolve({ default: body }))
    expect(component).toBe(body)
  })

  it('rejects a bundle naming an unknown contract version, before any load', async () => {
    const importModule = vi.fn()
    await expect(loadBundle({ ...FACT, contract: 99 }, importModule)).rejects.toThrow(
      /contract 99/,
    )
    expect(importModule).not.toHaveBeenCalled()
  })

  it('rejects a bundle with no component to render', async () => {
    await expect(loadBundle(FACT, () => Promise.resolve({}))).rejects.toThrow(
      /exports no component/,
    )
  })

  it('rejects a bundle the asset cannot deliver', async () => {
    await expect(
      loadBundle(FACT, () => Promise.reject(new TypeError('failed to fetch'))),
    ).rejects.toThrow(TypeError)
  })
})

describe('createBundleStore', () => {
  it('loads one entry once, however many attachment points ask', async () => {
    const importModule = vi.fn(() => Promise.resolve({ default: () => null }))
    const store = createBundleStore(importModule)
    await store.attempt(FACT)
    await store.attempt(FACT)
    expect(importModule).toHaveBeenCalledTimes(1)
  })

  it('a failed load is not re-attempted — the report names the type once', async () => {
    const importModule = vi.fn(() => Promise.reject(new Error('gone')))
    const store = createBundleStore(importModule)
    await expect(store.attempt(FACT)).rejects.toThrow('gone')
    await expect(store.attempt(FACT)).rejects.toThrow('gone')
    expect(importModule).toHaveBeenCalledTimes(1)
  })
})

describe('bodyTypeRefs', () => {
  const types = [{ type_ref: 'epsilon/widget', ui: FACT }, { type_ref: 'epsilon/source' }]

  it('a node on the canvas of a declaring type asks, nothing else does', () => {
    const graph = {
      nodes: [
        { uuid: 'u1', type_ref: 'epsilon/widget' },
        { uuid: 'u2', type_ref: 'epsilon/source' },
      ],
    }
    expect([...bodyTypeRefs(graph, types)]).toEqual(['epsilon/widget'])
    expect(
      bodyTypeRefs({ nodes: [{ uuid: 'u1', type_ref: 'epsilon/source' }] }, types).size,
    ).toBe(0)
  })

  it('a populated palette without a dropped node asks for nothing', () => {
    expect(bodyTypeRefs({ nodes: [] }, types).size).toBe(0)
  })
})

describe('valueTypeRefs', () => {
  const types = [
    {
      type_ref: 'epsilon/source',
      outputs: [
        { name: 'tone', type_refs: ['epsilon/tone'] },
        { name: 'level', type_refs: ['i32'] },
      ],
    },
  ]
  const dataTypes = {
    'epsilon/tone': { color: '#4a90d9', shape: 'circle', ui: FACT },
    i32: { color: '#2d72d2', shape: 'square' },
  }

  it('a displayed value at a single-typed port declaring value UI asks', () => {
    const graph = { nodes: [{ uuid: 'u1', type_ref: 'epsilon/source' }] }
    const refs = valueTypeRefs(graph, types, dataTypes, { 'u1/tone': '440 Hz', 'u1/level': '3' })
    expect([...refs]).toEqual(['epsilon/tone'])
  })

  it('a scalar value, and nothing displayed, ask for nothing', () => {
    const graph = { nodes: [{ uuid: 'u1', type_ref: 'epsilon/source' }] }
    expect(valueTypeRefs(graph, types, dataTypes, { 'u1/level': '3' }).size).toBe(0)
    expect(valueTypeRefs(graph, types, dataTypes, {}).size).toBe(0)
  })
})

const WIDGET = {
  node: { uuid: 'u1', type_ref: 'epsilon/widget', parameters: { level: 3 } },
  type: { type_ref: 'epsilon/widget', label: 'Widget', icon: '<svg/>', inputs: [], outputs: [] },
  wiredInputs: [],
  scalarInputs: [],
  marks: [],
  status: undefined,
  portValues: {},
  dataTypes: {},
}

function renderNode(data, context, edit = vi.fn()) {
  return render(
    <ReactFlowProvider>
      <PluginUiContext.Provider value={{ h: createElement, ...context }}>
        <EditContext.Provider value={edit}>
          <TypeNode data={data} selected={false} />
        </EditContext.Provider>
      </PluginUiContext.Provider>
    </ReactFlowProvider>,
  )
}

describe('TypeNode with custom UI', () => {
  it('renders the plugin body between the title bar and the ports, with the contract as props', () => {
    let seen = null
    const Body = (props) => {
      seen = props
      return <div data-testid="body" />
    }
    const { container } = renderNode(
      {
        ...WIDGET,
        status: 'running',
        marks: ['a problem'],
        portValues: { out: '440 Hz' },
        type: { ...WIDGET.type, outputs: [{ name: 'out', type_refs: ['tone'] }] },
      },
      { bodies: { 'epsilon/widget': Body } },
    )
    expect(screen.getByTestId('body')).not.toBeNull()
    expect(container.querySelector('.node-title')).not.toBeNull()
    expect(container.querySelector('.node-ports')).not.toBeNull()
    expect(seen).toEqual({
      h: createElement,
      label: 'Widget',
      parameters: { level: 3 },
      connectedInputs: [],
      status: 'running',
      marks: ['a problem'],
      portValues: { out: '440 Hz' },
      locked: false,
      setLabel: expect.any(Function),
      setParameter: expect.any(Function),
    })
  })

  it('the label is the stored override else the type default', () => {
    let seen = null
    const Body = (props) => {
      seen = props
      return null
    }
    renderNode(
      { ...WIDGET, node: { ...WIDGET.node, label: 'The widget' } },
      { bodies: { 'epsilon/widget': Body } },
    )
    expect(seen.label).toBe('The widget')
    cleanup()
    renderNode(WIDGET, { bodies: { 'epsilon/widget': Body } })
    expect(seen.label).toBe('Widget')
  })

  it('setLabel and setParameter ride the ordinary operations, an empty parameter unsetting', () => {
    const edit = vi.fn()
    let seen = null
    const Body = (props) => {
      seen = props
      return null
    }
    renderNode(WIDGET, { bodies: { 'epsilon/widget': Body } }, edit)
    seen.setLabel('renamed')
    expect(edit).toHaveBeenCalledWith('set_label', { uuid: 'u1', label: 'renamed' })
    seen.setParameter('level', '9')
    expect(edit).toHaveBeenCalledWith('set_parameter', { uuid: 'u1', input: 'level', value: '9' })
    seen.setParameter('level', '')
    expect(edit).toHaveBeenCalledWith('set_parameter', { uuid: 'u1', input: 'level' })
  })

  it('without a loaded bundle the node renders by the default class', () => {
    const { container } = renderNode(WIDGET, {})
    expect(container.querySelector('.node-body')).toBeNull()
    expect(container.querySelector('.node-ports')).not.toBeNull()
  })

  it('a body that throws while rendering the live state falls back and reports, naming the type', () => {
    const failBody = vi.fn()
    const Body = () => {
      throw new Error('cannot render the run state')
    }
    const { container } = renderNode(
      WIDGET,
      { bodies: { 'epsilon/widget': Body }, failBody },
    )
    expect(container.querySelector('.node-body')).toBeNull()
    expect(container.querySelector('.node-ports')).not.toBeNull()
    expect(failBody).toHaveBeenCalledWith('epsilon/widget', expect.any(Error))
  })
})

describe('the port readout of a value with declared UI', () => {
  const toneTypes = { tone: { color: '#4a90d9', shape: 'circle', ui: FACT } }
  const scalarTypes = { i32: { color: '#2d72d2', shape: 'square' } }
  const data = (dataTypes, portValues) => ({
    ...WIDGET,
    dataTypes,
    type: { ...WIDGET.type, outputs: [{ name: 'out', type_refs: [Object.keys(dataTypes)[0]] }] },
    portValues,
  })

  it('renders through the plugin component once its bundle loads', () => {
    const Body = ({ value }) => <em>{value}</em>
    const { container } = renderNode(
      data(toneTypes, { out: '440 Hz' }),
      { valueBodies: { tone: Body } },
    )
    expect(container.querySelector('.port-value').textContent).toBe('440 Hz')
  })

  it('a declared bundle that has not loaded — or failed — leaves the readout contentless', () => {
    const { container } = renderNode(data(toneTypes, { out: '440 Hz' }), {})
    expect(container.querySelector('.port-value')).toBeNull()
  })

  it('a value body that throws is reported naming the type, the readout falling back', () => {
    const failValue = vi.fn()
    const Body = () => {
      throw new Error('bad render')
    }
    const { container } = renderNode(
      data(toneTypes, { out: '440 Hz' }),
      { valueBodies: { tone: Body }, failValue },
    )
    expect(container.querySelector('.port-value')).toBeNull()
    expect(failValue).toHaveBeenCalledWith('tone', expect.any(Error))
  })

  it('a scalar value keeps its plain text readout', () => {
    const { container } = renderNode(data(scalarTypes, { out: '7' }), {})
    expect(container.querySelector('.port-value').textContent).toBe('7')
  })
})
