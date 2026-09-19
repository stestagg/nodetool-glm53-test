/** @vitest-environment jsdom */
// The packaging gestures end to end, the browser against a fake server
// seam: the menu on the selection, the dialog with its suggestion, the
// operation the commit sends, the collapsed node the push renders with
// the wires landed on its ports and the selection the reply hands it,
// the unpack restoring nodes and wires the same way — and the run lock
// taking both gestures away while selection stays live.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { ReactFlowProvider } from '@xyflow/react'
import { Editor } from './editor.jsx'
import { connect } from './protocol.js'

vi.mock('./protocol.js', () => ({
  NODE_TYPE: 'application/x-nodetool-node-type',
  connectionLost: { connectionLost: true },
  connect: vi.fn(),
}))

// React Flow measures its panes; jsdom has no layout engine to measure with.
window.ResizeObserver = window.ResizeObserver ?? class { observe() {} unobserve() {} disconnect() {} }

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

const SOURCE = '00000000-0000-0000-0000-000000000001'
const FIRST = '00000000-0000-0000-0000-000000000002'
const SINK = '00000000-0000-0000-0000-000000000003'
const INSTANCE = '00000000-0000-0000-0000-00000000000a'
const SECOND = '00000000-0000-0000-0000-00000000000e'
const BODY_1 = '00000000-0000-0000-0000-00000000000b'
const BODY_2 = '00000000-0000-0000-0000-00000000000c'

const LISTING = {
  types: [
    {
      type_ref: 'alpha/add',
      label: 'Add',
      icon: null,
      inputs: [
        { name: 'a', type_refs: ['i32'] },
        { name: 'b', type_refs: ['i32'] },
      ],
      outputs: [{ name: 'sum', type_refs: ['i32'] }],
    },
  ],
  baseScalars: { i32: true },
  dataTypes: { i32: { color: '#2d72d2', shape: 'square' } },
}

// A two-node stage with a wire crossing its boundary once the sink is
// drawn: the selection carries the adds, the sink stays outside.
const graphWith = (nodes, edges, groups = []) => ({
  nodes: [
    { uuid: SOURCE, type_ref: 'alpha/add', metadata: { position: { x: 0, y: 0 } } },
    { uuid: FIRST, type_ref: 'alpha/add', metadata: { position: { x: 100, y: 0 } } },
    ...nodes,
  ],
  edges,
  groups,
})

// The editor against a recorded seam: the connect hooks, the requests
// the page has sent — each answered by the test — and the pushes the
// server would send, delivered through the same hooks.
function editor(graph, run = { running: false }) {
  const requests = []
  const hooks = { current: null }
  connect.mockImplementation((received) => {
    hooks.current = received
    return () => {}
  })
  render(
    <ReactFlowProvider>
      <Editor />
    </ReactFlowProvider>,
  )
  const request = (type, fields = {}) => {
    const entry = { type, fields }
    requests.push(entry)
    return new Promise((resolve, reject) => {
      entry.resolve = resolve
      entry.reject = reject
    })
  }
  hooks.current.onOpen(request)
  hooks.current.onGreeting()
  requests.find((entry) => entry.type === 'list_node_types').resolve(LISTING)
  requests
    .find((entry) => entry.type === 'get_definition')
    .resolve({ graph, file: { path: null, dirty: false }, run, problems: [] })
  const push = (next) => {
    requests.length = 0
    hooks.current.onDefinition(next.graph, next.file ?? { path: null, dirty: true }, next.run ?? run, [])
  }
  return { requests, push, hooks }
}

// The drawn canvas node, held as an element: React Flow reuses the DOM
// node across a rebuild, so a reference taken once survives the pushes
// that ride the gestures.
const drawn = async (uuid) => waitFor(() => {
  const element = document.querySelector(`[data-id="${uuid}"]`)
  expect(element).toBeTruthy()
  return element
})

async function selectBoth() {
  fireEvent.click(await drawn(SOURCE), { detail: 1 })
  fireEvent.keyDown(await drawn(FIRST), { key: 'Enter', shiftKey: true })
  await waitFor(() => expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true))
}

const contextMenuOn = async (uuid) => {
  fireEvent.contextMenu(await drawn(uuid))
  await waitFor(() => screen.getByRole('menu'))
}

async function packaged({ requests, push }) {
  await selectBoth()
  const prompt = vi.spyOn(window, 'prompt').mockReturnValue('stage')
  await contextMenuOn(FIRST)
  expect(document.querySelector(`[data-id="${SOURCE}"]`).classList.contains('selected')).toBe(true)
  fireEvent.click(screen.getByRole('menuitem', { name: 'Package into group…' }))
  const sent = requests.find((entry) => entry.type === 'package_group')
  expect(sent.fields).toEqual({ nodes: [SOURCE, FIRST], name: 'stage' })
  sent.resolve({ type: 'group_packaged', uuid: INSTANCE, name: 'stage' })
  push({
    graph: {
      nodes: [
        { uuid: SINK, type_ref: 'alpha/add', metadata: { position: { x: 400, y: 0 } } },
        { uuid: INSTANCE, type_ref: 'stage', metadata: { position: { x: 50, y: 0 } } },
      ],
      edges: [{ from: INSTANCE, from_port: 'sum', to: SINK, to_port: 'a' }],
      groups: [
        {
          name: 'stage',
          inputs: [{ name: 'a', type_refs: ['i32'], node: BODY_1, port: 'a' }],
          outputs: [{ name: 'sum', type_refs: ['i32'], node: BODY_2, port: 'sum' }],
          nodes: [
            { uuid: BODY_1, type_ref: 'alpha/add', metadata: { position: { x: -50, y: 0 } } },
            { uuid: BODY_2, type_ref: 'alpha/add', metadata: { position: { x: 50, y: 0 } } },
          ],
          edges: [],
        },
      ],
    },
  })
  prompt.mockRestore()
  await drawn(INSTANCE)
  // The pick closed the menu that opened it; a fresh menu waits below.
  await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
  return sent
}

describe('the package gesture', () => {
  it('right-click on an unselected node selects it alone, the menu acting on the selection', async () => {
    await editor(graphWith([], []))
    await contextMenuOn(FIRST)
    expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true)
    expect(document.querySelector(`[data-id="${SOURCE}"]`).classList.contains('selected')).toBe(false)
  })

  it('the dialog opens with the available suggestion and Enter commits the operation', async () => {
    const session = editor(graphWith([], []))
    await contextMenuOn(FIRST)
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('Group')
    fireEvent.click(screen.getByRole('menuitem', { name: 'Package into group…' }))
    expect(prompt).toHaveBeenCalledWith('Package into group…', 'Group')
    expect(session.requests.find((entry) => entry.type === 'package_group').fields).toEqual({
      nodes: [FIRST],
      name: 'Group',
    })
  })

  it('a right-click that keeps a multi-selection packages the whole selection', async () => {
    const session = editor(graphWith([], []))
    await selectBoth()
    await contextMenuOn(FIRST)
    expect(document.querySelector(`[data-id="${SOURCE}"]`).classList.contains('selected')).toBe(true)
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('stage')
    fireEvent.click(screen.getByRole('menuitem', { name: 'Package into group…' }))
    expect(session.requests.find((entry) => entry.type === 'package_group').fields).toEqual({
      nodes: [SOURCE, FIRST],
      name: 'stage',
    })
  })

  it('the commit renders one collapsed node with the wires on its ports, and hands it the selection', async () => {
    const session = editor(graphWith([{ uuid: SINK, type_ref: 'alpha/add', metadata: {} }], [
      { from: FIRST, from_port: 'sum', to: SINK, to_port: 'a' },
    ]))
    await drawn(FIRST)
    await packaged(session)
    // The collapsed node carries the group's exposed ports — the seats the
    // crossing wires land on — and the selection the reply handed it. The
    // sidebar shows it as 20 provides for any selection: the exposed
    // input's own field, ready to feed.
    expect(screen.getByLabelText('stage a input')).toBeTruthy()
    expect(screen.getByLabelText('stage sum output')).toBeTruthy()
    expect(document.querySelector(`[data-id="${INSTANCE}"]`).classList.contains('selected')).toBe(true)
    expect(document.querySelector('.sidebar input[aria-label="stage a"]')).toBeTruthy()
  })

  it('Ctrl+G issues the same operation the menu entry sends', async () => {
    const session = editor(graphWith([], []))
    fireEvent.click(await drawn(FIRST), { detail: 1 })
    await waitFor(() => expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true))
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('stage')
    fireEvent.keyDown(document, { key: 'g', ctrlKey: true })
    const sent = session.requests.find((entry) => entry.type === 'package_group')
    expect(sent.fields).toEqual({ nodes: [FIRST], name: 'stage' })
  })

  it('a right-click on the bare pane with nothing selected opens no menu, and Escape still clears', async () => {
    await editor(graphWith([], []))
    await drawn(FIRST)
    fireEvent.contextMenu(document.querySelector('.react-flow__pane'))
    expect(screen.queryByRole('menu')).toBeNull()
    fireEvent.click(await drawn(FIRST), { detail: 1 })
    await waitFor(() => expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true))
    fireEvent.keyDown(document, { key: 'Escape' })
    await waitFor(() =>
      expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(false),
    )
  })

  it('a right-click on the pane acts on the standing selection', async () => {
    const session = editor(graphWith([], []))
    fireEvent.click(await drawn(FIRST), { detail: 1 })
    await waitFor(() => expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true))
    fireEvent.contextMenu(document.querySelector('.react-flow__pane'))
    await waitFor(() => screen.getByRole('menu'))
    expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true)
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('stage')
    fireEvent.click(screen.getByRole('menuitem', { name: 'Package into group…' }))
    expect(session.requests.find((entry) => entry.type === 'package_group').fields).toEqual({
      nodes: [FIRST],
      name: 'stage',
    })
  })

  it('Ctrl+G sends nothing while the menu is open; Escape closes it, the selection standing', async () => {
    const session = editor(graphWith([], []))
    await contextMenuOn(FIRST)
    fireEvent.keyDown(document, { key: 'g', ctrlKey: true })
    expect(session.requests.some((entry) => entry.type === 'package_group')).toBe(false)
    fireEvent.keyDown(document, { key: 'Escape' })
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true)
  })
})

describe('the unpack gesture', () => {
  it('dissolves the instance, restores the nodes and their wire, and hands them the selection', async () => {
    const session = editor(graphWith([], []))
    await drawn(FIRST)
    await packaged(session)

    await contextMenuOn(INSTANCE)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Unpack group' }))
    const sent = session.requests.find((entry) => entry.type === 'unpack_group')
    expect(sent.fields).toEqual({ uuid: INSTANCE })
    sent.resolve({ type: 'group_unpacked', nodes: [BODY_1, BODY_2] })
    session.push({
      graph: {
        nodes: [
          { uuid: SINK, type_ref: 'alpha/add', metadata: { position: { x: 400, y: 0 } } },
          { uuid: BODY_1, type_ref: 'alpha/add', metadata: { position: { x: 0, y: 0 } } },
          { uuid: BODY_2, type_ref: 'alpha/add', metadata: { position: { x: 100, y: 0 } } },
        ],
        edges: [{ from: BODY_2, from_port: 'sum', to: SINK, to_port: 'a' }],
        groups: [],
      },
    })
    await drawn(BODY_1)
    expect(document.querySelector(`[data-id="${BODY_1}"]`).classList.contains('selected')).toBe(true)
    expect(document.querySelector(`[data-id="${BODY_2}"]`).classList.contains('selected')).toBe(true)
    expect(document.querySelector(`[data-id="${INSTANCE}"]`)).toBeNull()
  })

  it('Ctrl+Shift+G unpacks the instances the menu entry would', async () => {
    const session = editor(graphWith([], []))
    await drawn(FIRST)
    await packaged(session)
    fireEvent.keyDown(document, { key: 'g', ctrlKey: true, shiftKey: true })
    const sent = session.requests.find((entry) => entry.type === 'unpack_group')
    expect(sent.fields).toEqual({ uuid: INSTANCE })
  })

  it('a refused unpack in a fan-out hands the selection to the nodes that returned, only', async () => {
    const session = editor(graphWith([], []))
    await drawn(FIRST)
    await packaged(session)
    // A second instance joins the selection: one unpack operation per
    // instance, and the second is refused mid-fan-out.
    session.push({
      graph: {
        nodes: [
          { uuid: SINK, type_ref: 'alpha/add', metadata: { position: { x: 400, y: 0 } } },
          { uuid: INSTANCE, type_ref: 'stage', metadata: { position: { x: 50, y: 0 } } },
          { uuid: SECOND, type_ref: 'stage', metadata: { position: { x: 200, y: 0 } } },
        ],
        edges: [],
        groups: [{ name: 'stage', inputs: [], outputs: [], nodes: [], edges: [] }],
      },
    })
    fireEvent.click(await drawn(INSTANCE), { detail: 1 })
    fireEvent.keyDown(await drawn(SECOND), { key: 'Enter', shiftKey: true })
    await waitFor(() => expect(document.querySelector(`[data-id="${SECOND}"]`).classList.contains('selected')).toBe(true))
    fireEvent.keyDown(document, { key: 'g', ctrlKey: true, shiftKey: true })
    const sent = session.requests.filter((entry) => entry.type === 'unpack_group')
    expect(sent).toHaveLength(2)
    sent[0].resolve({ type: 'group_unpacked', nodes: [BODY_1, BODY_2] })
    sent[1].reject('the run took the lock')
    session.push({
      graph: {
        nodes: [
          { uuid: SINK, type_ref: 'alpha/add', metadata: { position: { x: 400, y: 0 } } },
          { uuid: BODY_1, type_ref: 'alpha/add', metadata: { position: { x: 0, y: 0 } } },
          { uuid: BODY_2, type_ref: 'alpha/add', metadata: { position: { x: 100, y: 0 } } },
          { uuid: SECOND, type_ref: 'stage', metadata: { position: { x: 200, y: 0 } } },
        ],
        edges: [],
        groups: [{ name: 'stage', inputs: [], outputs: [], nodes: [], edges: [] }],
      },
    })
    // The nodes the one fulfilled operation returned take the selection;
    // the refusal contributes nothing to it — no non-uuid in the handoff,
    // no handoff stuck failing every later push.
    await waitFor(() =>
      expect(document.querySelector(`[data-id="${BODY_1}"]`).classList.contains('selected')).toBe(true),
    )
    expect(document.querySelector(`[data-id="${BODY_2}"]`).classList.contains('selected')).toBe(true)
    expect(document.querySelector(`[data-id="${SECOND}"]`).classList.contains('selected')).toBe(false)
  })

  it('an all-refused fan-out hands nothing over, the selection standing', async () => {
    const session = editor(graphWith([], []))
    await drawn(FIRST)
    await packaged(session)
    await contextMenuOn(INSTANCE)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Unpack group' }))
    const sent = session.requests.find((entry) => entry.type === 'unpack_group')
    sent.reject('the run took the lock')
    await waitFor(() => expect(screen.queryByRole('menu')).toBeNull())
    expect(document.querySelector(`[data-id="${INSTANCE}"]`).classList.contains('selected')).toBe(true)
  })
})

describe('the run lock takes the gestures away', () => {
  it('no menu opens on a right-click, the chord sends nothing, selection stays live', async () => {
    const session = editor(graphWith([], []), { running: true })
    fireEvent.contextMenu(await drawn(FIRST))
    expect(screen.queryByRole('menu')).toBeNull()
    expect(document.querySelector(`[data-id="${FIRST}"]`).classList.contains('selected')).toBe(true)
    const prompt = vi.spyOn(window, 'prompt').mockReturnValue('stage')
    fireEvent.keyDown(document, { key: 'g', ctrlKey: true })
    expect(session.requests.some((entry) => entry.type === 'package_group')).toBe(false)
  })
})
