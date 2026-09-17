// The editor shell: a react-flow style canvas under light Blueprint
// chrome, with the palette of every linked plugin's node types docked on
// the left and the editing sidebar docked on the right while a node is
// selected. The browser holds no authoritative graph state — it requests
// the listing and the definition on load, renders what the server holds,
// and every edit lands server-side and is pushed back. Drags follow the
// pointer locally; one operation commits where a dragged node rests,
// where a wire lands, which node a key press deletes, and what a field
// commit holds — the sidebar and the node's inline fields both committing
// through the same seam, both views of one stored value.

import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Background,
  ReactFlow,
  applyNodeChanges,
  useReactFlow,
} from '@xyflow/react'
import { NODE_TYPE, connect } from './protocol.js'
import { EditContext, scalarPossible } from './fields.jsx'
import { SelfLoopEdge } from './edges.jsx'
import { PlaceholderNode, TypeNode } from './nodes.jsx'
import { Sidebar } from './sidebar.jsx'

const nodeTypes = { type: TypeNode, placeholder: PlaceholderNode }
const edgeTypes = { selfloop: SelfLoopEdge }

// The fixed grid a node without a recorded position lands on, ordered by
// uuid: a deterministic, view-local fallback every view and reload agrees
// on, with nothing written into the definition.
const FALLBACK_PITCH = { x: 180, y: 140 }

function recordedPosition(node) {
  const position = node.metadata?.position
  if (typeof position?.x === 'number' && typeof position?.y === 'number') {
    return { x: position.x, y: position.y }
  }
  return undefined
}

export function toNodes(graph, types, baseScalars) {
  const byRef = new Map((types ?? []).map((type) => [type.type_ref, type]))
  const placeless = graph.nodes
    .filter((node) => recordedPosition(node) === undefined)
    .sort((a, b) => (a.uuid < b.uuid ? -1 : 1))
    .map((node, index) => [
      node.uuid,
      {
        x: 80 + (index % 5) * FALLBACK_PITCH.x,
        y: 80 + Math.floor(index / 5) * FALLBACK_PITCH.y,
      },
    ])
  const fallback = new Map(placeless)
  const wiredInputs = new Map()
  for (const edge of graph.edges) {
    const ports = wiredInputs.get(edge.to) ?? []
    ports.push(edge.to_port)
    wiredInputs.set(edge.to, ports)
  }
  return graph.nodes.map((node) => {
    const position = recordedPosition(node) ?? fallback.get(node.uuid)
    const type = byRef.get(node.type_ref)
    if (type === undefined) {
      return {
        id: node.uuid,
        position,
        type: 'placeholder',
        data: { label: node.label ?? node.type_ref },
        // Ports unknown, so no dragging or deletion — but the label is
        // the one attribute its sidebar can edit, so selection stays.
        draggable: false,
        selectable: true,
        deletable: false,
      }
    }
    return {
      id: node.uuid,
      position,
      type: 'type',
      data: {
        node,
        type,
        wiredInputs: wiredInputs.get(node.uuid) ?? [],
        scalarInputs: type.inputs
          .filter((port) => scalarPossible(port, baseScalars))
          .map((port) => port.name),
      },
    }
  })
}

// The definition's edges as canvas wires. Not selectable: drag-off is the
// one way a wire comes off, so there is no second, selected-then-deleted
// path.
export function toEdges(graph) {
  return graph.edges.map((edge) => ({
    id: `${edge.from}/${edge.from_port}->${edge.to}/${edge.to_port}`,
    source: edge.from,
    sourceHandle: edge.from_port,
    target: edge.to,
    targetHandle: edge.to_port,
    selectable: false,
    type: edge.from === edge.to ? 'selfloop' : undefined,
  }))
}

// What a wire drag's ending means when no connection landed: a release on
// a port — any port, even one the wire cannot land on, the wire's own
// included — is the wire gesture and cancels quietly; a connected input's
// end released anywhere that is not a port unhooks it. React Flow reports
// whatever port sits under the release, valid or not, as `toHandle`.
// Answers the unhook operation's fields, or null.
export function offPortUnhook(edges, state) {
  const from = state.fromHandle
  if (from === null || from.type !== 'target' || state.toHandle != null) return null
  const wired = edges.some(
    (edge) => edge.to === from.nodeId && edge.to_port === from.id,
  )
  return wired ? { to: from.nodeId, to_port: from.id } : null
}

// Whether the held graph carries unsaved changes — the one condition the
// discard guard asks under, silence there being the one way to lose work.
export function hasUnsavedChanges(file) {
  return file?.dirty === true
}

// Whether a save asks for its target: saving elsewhere names a path, and
// so does the first save of an untitled graph; any other save writes the
// file being edited without asking.
export function saveAsksForPath(file, elsewhere) {
  return elsewhere || file?.path == null
}

export function Editor() {
  const [listing, setListing] = useState(null)
  const [graph, setGraph] = useState(null)
  const [file, setFile] = useState(null)
  const [nodes, setNodes] = useState([])
  const [status, setStatus] = useState({ text: 'connecting…', error: false })
  const protocol = useRef(null)
  const canvasRef = useRef(null)
  // Armed by a replacement gesture — open, new — and consumed by the
  // next definition arrival, which brings the view to the graph; a
  // failed gesture disarms it.
  const pendingRefit = useRef(false)
  const { screenToFlowPosition, getViewport, setViewport, fitView } = useReactFlow()

  // An error is the one message the user must not miss; the status line
  // paints it red.
  const showError = useCallback((text) => {
    setStatus({ text, error: true })
  }, [])

  useEffect(() => {
    if (graph === null) return
    // Selection and a drag in flight are view state: the definition push
    // replaces what is drawn, never what the user has picked or holds.
    setNodes((current) => {
      const before = new Map(current.map((node) => [node.id, node]))
      return toNodes(graph, listing?.types, listing?.baseScalars).map((node) => {
        const held = before.get(node.id)
        return {
          ...node,
          selected: held?.selected ?? false,
          position: held?.dragging ? held.position : node.position,
        }
      })
    })
    // A replacement brings the view to the graph that arrived, the way
    // the launch fit does; the queued fit waits for the new nodes to be
    // measured. An ordinary edit push leaves the view to the user.
    if (pendingRefit.current) {
      pendingRefit.current = false
      fitView({ maxZoom: 1 })
    }
  }, [graph, listing, fitView])

  useEffect(
    () =>
      connect({
        onOpen: (request) => {
          protocol.current = request
          request('list_node_types').then(setListing, showError)
          request('get_definition').then(({ graph, file }) => {
            setGraph(graph)
            setFile(file)
          }, showError)
        },
        onGreeting: () => setStatus({ text: '' }),
        onDefinition: (graph, file) => {
          setGraph(graph)
          setFile(file)
        },
        onFile: ({ path, dirty }) => setFile({ path, dirty }),
        onError: showError,
        onClosed: () =>
          setStatus({ text: 'connection lost — reload the page', error: true }),
      }),
    [showError],
  )

  // The editing seam every field commit rides: one operation out, the
  // definition push re-rendering both views. An error — a commit for a
  // since-connected input, say — lands in the status line, the definition
  // untouched.
  const edit = useCallback((type, fields) => {
    protocol.current?.(type, fields)?.catch(showError)
  }, [showError])

  const onNodesChange = useCallback((changes) => {
    // A remove change is the server's to apply: the delete operation goes
    // out, the definition push takes the node and its wires off the
    // canvas, and the view never holds a deletion the server refused.
    for (const change of changes) {
      if (change.type === 'remove' && protocol.current !== null) {
        protocol.current('delete_node', { uuid: change.id }).catch(showError)
      }
    }
    setNodes((current) =>
      applyNodeChanges(
        changes.filter((change) => change.type !== 'remove'),
        current,
      ),
    )
  }, [showError])

  const onNodeDragStop = useCallback((_event, node) => {
    if (node.type === 'placeholder' || protocol.current === null) return
    protocol.current('move_node', {
      uuid: node.id,
      position: { x: node.position.x, y: node.position.y },
    }).catch(showError)
  }, [showError])

  const onConnect = useCallback((connection) => {
    if (protocol.current === null) return
    protocol.current('wire', {
      from: connection.source,
      from_port: connection.sourceHandle,
      to: connection.target,
      to_port: connection.targetHandle,
    }).catch(showError)
  }, [showError])

  const onConnectEnd = useCallback(
    (_event, state) => {
      const unhook = offPortUnhook(graph?.edges ?? [], state)
      if (unhook !== null && protocol.current !== null) {
        protocol.current('unhook', unhook).catch(showError)
      }
    },
    [graph, showError],
  )

  const onDrop = useCallback(
    (event) => {
      event.preventDefault()
      const typeRef = event.dataTransfer.getData(NODE_TYPE)
      if (typeRef === '' || protocol.current === null) return
      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY })
      protocol
        .current('create_node', { type_ref: typeRef, position })
        .catch(showError)
    },
    [screenToFlowPosition, showError],
  )

  const onDragOver = useCallback((event) => {
    event.preventDefault()
    event.dataTransfer.dropEffect = 'move'
  }, [])

  // The one guard over loss: replacing the held graph — opening another
  // file or the same one, or starting fresh — asks before discarding
  // unsaved changes, and nowhere else.
  const confirmDiscard = useCallback(() => {
    if (!hasUnsavedChanges(file)) return true
    return window.confirm('The graph has unsaved changes. Discard them?')
  }, [file])

  const newGraph = useCallback(() => {
    if (protocol.current === null || !confirmDiscard()) return
    pendingRefit.current = true
    protocol.current('new_graph').catch((error) => {
      pendingRefit.current = false
      showError(error)
    })
  }, [confirmDiscard, showError])

  const openFile = useCallback(() => {
    if (protocol.current === null) return
    // The path first, prefilled with the current one so a sibling file is
    // an edit in place: a cancelled or empty answer ends the open before
    // the discard confirm collects an answer about a loss that never
    // followed.
    const path = window.prompt('Open graph file', file?.path ?? '')
    if (path === null || path === '') return
    if (!confirmDiscard()) return
    pendingRefit.current = true
    protocol
      .current('open_file', { path })
      .then(() =>
        // The opened graph replaces the one any selection points into.
        setNodes((current) =>
          current.map((node) => ({ ...node, selected: false })),
        ),
      )
      .catch((error) => {
        pendingRefit.current = false
        showError(error)
      })
  }, [confirmDiscard, file, showError])

  // One save mechanism under both chrome entries: a save always knows its
  // target — the current file — and asks only for the first save of an
  // untitled graph or when saving elsewhere; the answer becomes the file
  // once the save succeeds.
  const save = useCallback(
    (elsewhere) => {
      if (protocol.current === null) return
      if (saveAsksForPath(file, elsewhere)) {
        const path = window.prompt('Save graph to', file?.path ?? '')
        // An empty answer is a stray Enter, not a target.
        if (path === null || path === '') return
        protocol.current('save_file', { path }).catch(showError)
      } else {
        protocol.current('save_file').catch(showError)
      }
    },
    [file, showError],
  )

  // The sidebar's node: the one the canvas holds selected, read off the
  // same node state the canvas draws, so a definition push swaps its
  // contents with everything else.
  const selectedUuid = nodes.find((node) => node.selected)?.id ?? null
  const selected = graph?.nodes.find((node) => node.uuid === selectedUuid)
  const selectedWiredInputs = (graph?.edges ?? [])
    .filter((edge) => edge.to === selectedUuid)
    .map((edge) => edge.to_port)

  // Opening the dock shrinks the canvas beneath it, and a node sitting in
  // the lost width would vanish under the very panel it is being edited
  // in. When the selection lands covered, pan just enough that it clears
  // the dock's edge; an already-visible node stays where it is.
  useEffect(() => {
    if (selectedUuid === null) return
    const canvas = canvasRef.current
    const node = canvas?.querySelector(`[data-id="${selectedUuid}"]`)
    if (node === null || node === undefined) return
    const covered =
      node.getBoundingClientRect().right + 24 - canvas.getBoundingClientRect().right
    if (covered <= 0) return
    const view = getViewport()
    setViewport({ x: view.x - covered / view.zoom, y: view.y, zoom: view.zoom })
  }, [selectedUuid, getViewport, setViewport])

  const types = listing?.types
  const empty =
    graph !== null && graph.nodes.length === 0 && types !== undefined && types.length > 0

  return (
    <EditContext.Provider value={edit}>
      <div className="app">
        <header className="chrome">
          <span className="file-name">{file?.path ?? 'untitled'}</span>
          {hasUnsavedChanges(file) && (
            <span className="file-dirty">unsaved changes</span>
          )}
          <span className="chrome-space" />
          <button onClick={newGraph}>New</button>
          <button onClick={openFile}>Open</button>
          <button onClick={() => save(false)}>Save</button>
          <button onClick={() => save(true)}>Save as</button>
        </header>
        <div className="workspace">
          <aside className="palette">
            <h1 className="palette-title">Nodes</h1>
            {types === undefined ? null : types.length === 0 ? (
              <p className="palette-empty">
                No node types are linked into this binary. Link a plugin crate
                to see its types here.
              </p>
            ) : (
              <ul className="palette-list">
                {types.map((type) => (
                  <li
                    key={type.type_ref}
                    className="palette-item"
                    draggable
                    onDragStart={(event) => {
                      event.dataTransfer.setData(NODE_TYPE, type.type_ref)
                      event.dataTransfer.effectAllowed = 'move'
                    }}
                  >
                    <div className="palette-label">{type.label}</div>
                    <div className="palette-plugin">{type.plugin}</div>
                  </li>
                ))}
              </ul>
            )}
          </aside>
          <main className="canvas" ref={canvasRef}>
            <ReactFlow
              nodes={nodes}
              edges={graph === null ? [] : toEdges(graph)}
              nodeTypes={nodeTypes}
              edgeTypes={edgeTypes}
              onNodesChange={onNodesChange}
              onNodeDragStop={onNodeDragStop}
              onConnect={onConnect}
              onConnectEnd={onConnectEnd}
              onDrop={onDrop}
              onDragOver={onDragOver}
              fitView
              // The initial fit never zooms in past 100%: fitting a small
              // graph up to maxZoom would lurch the view under the pointer.
              fitViewOptions={{ maxZoom: 1 }}
              minZoom={0.25}
              maxZoom={2.5}
              // Delete/Backspace is the deletion gesture. A wire is a drag
              // from either end — a click never starts or lands one — and
              // the drag threshold keeps a port click from reading as a
              // drag-off.
              deleteKeyCode={['Delete', 'Backspace']}
              connectionDragThreshold={4}
              connectOnClick={false}
            >
              <Background variant="dots" gap={24} size={1.5} />
            </ReactFlow>
            {empty && (
              <div className="canvas-hint">
                Drag a node type from the palette onto the canvas.
              </div>
            )}
          </main>
          <Sidebar
            node={selected}
            type={listing?.types.find((type) => type.type_ref === selected?.type_ref)}
            wiredInputs={selectedWiredInputs}
            baseScalars={listing?.baseScalars ?? {}}
          />
        </div>
        <footer className={`status${status.error ? ' error' : ''}`}>
          {status.text}
        </footer>
      </div>
    </EditContext.Provider>
  )
}
