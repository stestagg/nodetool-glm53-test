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

export function Editor() {
  const [listing, setListing] = useState(null)
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
      return toNodes(graph, listing?.types, listing?.baseScalars).map((node) => {
        const held = before.get(node.id)
        return {
          ...node,
          selected: held?.selected ?? false,
          position: held?.dragging ? held.position : node.position,
        }
      })
    })
  }, [graph, listing])

  useEffect(
    () =>
      connect({
        onOpen: (request) => {
          protocol.current = request
          request('list_node_types').then(setListing, setStatus)
          request('get_definition').then(setGraph, setStatus)
        },
        onGreeting: () => setStatus(''),
        onDefinition: setGraph,
        onError: setStatus,
        onClosed: () => setStatus('connection lost — reload the page'),
      }),
    [],
  )

  // The editing seam every field commit rides: one operation out, the
  // definition push re-rendering both views. An error — a commit for a
  // since-connected input, say — lands in the status line, the definition
  // untouched.
  const edit = useCallback((type, fields) => {
    protocol.current?.(type, fields)?.catch(setStatus)
  }, [])

  const onNodesChange = useCallback((changes) => {
    // A remove change is the server's to apply: the delete operation goes
    // out, the definition push takes the node and its wires off the
    // canvas, and the view never holds a deletion the server refused.
    for (const change of changes) {
      if (change.type === 'remove' && protocol.current !== null) {
        protocol.current('delete_node', { uuid: change.id }).catch(setStatus)
      }
    }
    setNodes((current) =>
      applyNodeChanges(
        changes.filter((change) => change.type !== 'remove'),
        current,
      ),
    )
  }, [])

  const onNodeDragStop = useCallback((_event, node) => {
    if (node.type === 'placeholder' || protocol.current === null) return
    protocol.current('move_node', {
      uuid: node.id,
      position: { x: node.position.x, y: node.position.y },
    }).catch(setStatus)
  }, [])

  const onConnect = useCallback((connection) => {
    if (protocol.current === null) return
    protocol.current('wire', {
      from: connection.source,
      from_port: connection.sourceHandle,
      to: connection.target,
      to_port: connection.targetHandle,
    }).catch(setStatus)
  }, [])

  const onConnectEnd = useCallback(
    (_event, state) => {
      const unhook = offPortUnhook(graph?.edges ?? [], state)
      if (unhook !== null && protocol.current !== null) {
        protocol.current('unhook', unhook).catch(setStatus)
      }
    },
    [graph],
  )

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

  // The sidebar's node: the one the canvas holds selected, read off the
  // same node state the canvas draws, so a definition push swaps its
  // contents with everything else.
  const selectedUuid = nodes.find((node) => node.selected)?.id ?? null
  const selected = graph?.nodes.find((node) => node.uuid === selectedUuid)
  const selectedWiredInputs = (graph?.edges ?? [])
    .filter((edge) => edge.to === selectedUuid)
    .map((edge) => edge.to_port)

  const types = listing?.types
  const empty =
    graph !== null && graph.nodes.length === 0 && types !== undefined && types.length > 0

  return (
    <EditContext.Provider value={edit}>
      <div className="app">
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
          <main className="canvas">
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
            edit={edit}
          />
        </div>
        <footer className="status">{status}</footer>
      </div>
    </EditContext.Provider>
  )
}
