// The editor shell: a react-flow style canvas under light Blueprint
// chrome, with the palette of every linked plugin's node types docked on
// the left and the right edge kept free for the editing sidebar. The
// browser holds no authoritative graph state — it requests the definition
// on load, renders what the server holds, and every edit lands server-side
// and is pushed back. Drags follow the pointer locally; one operation
// commits where a dragged node rests.

import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Background,
  ReactFlow,
  applyNodeChanges,
  useReactFlow,
} from '@xyflow/react'
import { NODE_TYPE, connect } from './protocol.js'
import { PlaceholderNode, TypeNode } from './nodes.jsx'

const nodeTypes = { type: TypeNode, placeholder: PlaceholderNode }

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

export function toNodes(graph, types) {
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
  return graph.nodes.map((node) => {
    const position = recordedPosition(node) ?? fallback.get(node.uuid)
    const type = byRef.get(node.type_ref)
    if (type === undefined) {
      return {
        id: node.uuid,
        position,
        type: 'placeholder',
        data: { label: node.label ?? node.type_ref },
        draggable: false,
        selectable: false,
        deletable: false,
      }
    }
    return { id: node.uuid, position, type: 'type', data: { node, type } }
  })
}

export function Editor() {
  const [types, setTypes] = useState(null)
  const [graph, setGraph] = useState(null)
  const [nodes, setNodes] = useState([])
  const [status, setStatus] = useState('connecting…')
  const protocol = useRef(null)
  const { screenToFlowPosition } = useReactFlow()

  useEffect(() => {
    if (graph === null) return
    // Selection and a drag in flight are view state: the definition push
    // replaces what is drawn, never what the user has picked or holds.
    setNodes((current) => {
      const before = new Map(current.map((node) => [node.id, node]))
      return toNodes(graph, types).map((node) => {
        const held = before.get(node.id)
        return {
          ...node,
          selected: held?.selected ?? false,
          position: held?.dragging ? held.position : node.position,
        }
      })
    })
  }, [graph, types])

  useEffect(
    () =>
      connect({
        onOpen: (request) => {
          protocol.current = request
          request('list_node_types').then(setTypes, setStatus)
          request('get_definition').then(setGraph, setStatus)
        },
        onGreeting: () => setStatus(''),
        onDefinition: setGraph,
        onError: setStatus,
        onClosed: () => setStatus('connection lost — reload the page'),
      }),
    [],
  )

  const onNodesChange = useCallback((changes) => {
    setNodes((current) => applyNodeChanges(changes, current))
  }, [])

  const onNodeDragStop = useCallback((_event, node) => {
    if (node.type === 'placeholder' || protocol.current === null) return
    protocol.current('move_node', {
      uuid: node.id,
      position: { x: node.position.x, y: node.position.y },
    }).catch(setStatus)
  }, [])

  const onDrop = useCallback(
    (event) => {
      event.preventDefault()
      const typeRef = event.dataTransfer.getData(NODE_TYPE)
      if (typeRef === '' || protocol.current === null) return
      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY })
      protocol
        .current('create_node', { type_ref: typeRef, position })
        .catch(setStatus)
    },
    [screenToFlowPosition],
  )

  const onDragOver = useCallback((event) => {
    event.preventDefault()
    event.dataTransfer.dropEffect = 'move'
  }, [])

  const empty =
    graph !== null && graph.nodes.length === 0 && types !== null && types.length > 0

  return (
    <div className="app">
      <div className="workspace">
        <aside className="palette">
          <h1 className="palette-title">Nodes</h1>
          {types === null ? null : types.length === 0 ? (
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
        <main className="canvas">
          <ReactFlow
            nodes={nodes}
            edges={[]}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onNodeDragStop={onNodeDragStop}
            onDrop={onDrop}
            onDragOver={onDragOver}
            fitView
            // The initial fit never zooms in past 100%: fitting a small
            // graph up to maxZoom would lurch the view under the pointer.
            fitViewOptions={{ maxZoom: 1 }}
            minZoom={0.25}
            maxZoom={2.5}
          >
            <Background variant="dots" gap={24} size={1.5} />
          </ReactFlow>
          {empty && (
            <div className="canvas-hint">
              Drag a node type from the palette onto the canvas.
            </div>
          )}
        </main>
      </div>
      <footer className="status">{status}</footer>
    </div>
  )
}
