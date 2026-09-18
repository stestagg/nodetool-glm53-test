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
//
// Every report arrives through one surface: a toast in the chrome,
// dismissed on click and on its own. What toasts deliberately do not
// carry is the durable truth — a problem a compile found sits as a mark
// on the node it names, a failed run's explanation sits on the failed
// node, and both are gone when the problem or the next run is. A lost
// connection is chrome state of its own: a banner naming the loss over
// the last-known canvas, every server-acting gesture inert while the
// looking stays live, the tab rejoining by itself when the server
// returns.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Background,
  ReactFlow,
  applyNodeChanges,
  useReactFlow,
} from '@xyflow/react'
import { NODE_TYPE, connect } from './protocol.js'
import { EditContext, LockContext, scalarPossible } from './fields.jsx'
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

export function toNodes(graph, types, baseScalars, marks) {
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
  const at = (uuid) => marks?.get(uuid) ?? []
  return graph.nodes.map((node) => {
    const position = recordedPosition(node) ?? fallback.get(node.uuid)
    const type = byRef.get(node.type_ref)
    if (type === undefined) {
      return {
        id: node.uuid,
        position,
        type: 'placeholder',
        data: { label: node.label ?? node.type_ref, marks: at(node.uuid) },
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
        marks: at(node.uuid),
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

// The chrome's one run control: it flips with the run state — Start when
// idle, Stop while running, no separate mode — and a start needs
// something to run, an empty definition leaving it disabled. Both acts
// reach the server, so a connection that cannot delivers them is the
// same as no control.
export function runControl(run, graph, connected = true) {
  const running = run?.running === true
  return {
    running,
    label: running ? 'Stop' : 'Start',
    enabled: connected && (running || (graph?.nodes.length ?? 0) > 0),
  }
}

// Where each problem lives, as the canvas draws it: one mark per node,
// carrying the messages of every problem that names it — the compile's
// attribution read as a structure, never parsed from messages — plus the
// failed run's error at the node whose failure ended it.
export function nodeMarks(problems, run) {
  const marks = new Map()
  const add = (uuid, message) => {
    const list = marks.get(uuid) ?? []
    list.push(message)
    marks.set(uuid, list)
  }
  for (const problem of problems ?? []) {
    for (const uuid of problem.nodes ?? []) add(uuid, problem.message)
  }
  if (run?.outcome === 'failed' && run.node) add(run.node, run.error)
  return marks
}

// The banner a connection state names, or none: the loss with the trying
// it is owed, the version mismatch with the reload that is the advice —
// never a self-dismissing toast. A connected or not-yet-lost editor has
// no banner.
export function bannerText(connection, mismatch) {
  if (connection === 'lost') return 'connection lost — trying to reconnect…'
  if (connection === 'incompatible') return `${mismatch} — reload the page`
  return null
}

// The status line's reading of the run state: the running state itself,
// or the last run's outcome, a failure naming what failed.
export function runStatusText(run) {
  if (run?.running === true) return 'running'
  if (run?.outcome === 'failed') return `run failed: ${run.error ?? 'unknown failure'}`
  if (run?.outcome) return `run ${run.outcome}`
  return ''
}

// How long a toast stays when nobody dismisses it.
const TOAST_MS = 8000

export function Editor() {
  const [listing, setListing] = useState(null)
  const [graph, setGraph] = useState(null)
  const [file, setFile] = useState(null)
  const [run, setRun] = useState(null)
  const [problems, setProblems] = useState([])
  const [nodes, setNodes] = useState([])
  const [status, setStatus] = useState({ text: 'connecting…', error: false })
  // The connection: connecting until the first greeting, open while it
  // holds, lost on every drop the client is retrying, and incompatible
  // when a greeting named a version mismatch — the one state the client
  // stops retrying from.
  const [connection, setConnection] = useState('connecting')
  const [mismatch, setMismatch] = useState(null)
  // The run control's click guard: armed by a start or stop, settled by
  // the run state landing. The control's flip travels through the
  // server's push, so a second click inside that gap — the second click
  // of a habitual double-click on Start, say — lands before the label
  // has flipped and must be ignored rather than read as the opposite act.
  const [acting, setActing] = useState(false)
  const [toasts, setToasts] = useState([])
  const protocol = useRef(null)
  const canvasRef = useRef(null)
  const toastId = useRef(0)
  // Armed by a replacement gesture — open, new — and consumed by the
  // next definition arrival, which brings the view to the graph; a
  // failed gesture disarms it.
  const pendingRefit = useRef(false)
  const { screenToFlowPosition, getViewport, setViewport, fitView } = useReactFlow()

  const connected = connection === 'open'
  // While a run is on the definition is held still, and while the
  // connection is gone nothing can reach the server: every editing
  // gesture goes quiet — palette drops, moves, wires, deletion, label
  // and parameter edits, and the file controls — while selection,
  // panning, and zoom stay live, looking not being editing. The server
  // refuses whatever slips through when the lock is the reason; when the
  // connection is, nothing is sent at all — the browser holds no graph
  // state that could back an undeliverable edit.
  const locked = run?.running === true
  const editable = connected && !locked

  // A toast is the one surface a transient report arrives through:
  // dismissed on click, and on its own — a happening, never the durable
  // truth, which lives in the marks and the chrome state.
  const dismissToast = useCallback((id) => {
    setToasts((current) => current.filter((toast) => toast.id !== id))
  }, [])
  const showToast = useCallback(
    (text) => {
      const id = toastId.current += 1
      setToasts((current) => [...current, { id, text }])
      setTimeout(() => dismissToast(id), TOAST_MS)
    },
    [dismissToast],
  )

  // A definition arrival — the connect-time resync or a push — replaces
  // what is drawn, and the run state it carries narrates itself the way a
  // run push does. An idle state with no outcome — what a compile failure
  // leaves — reads as '', leaving the status line, the refused start's
  // error report, as it is.
  const resync = useCallback((graph, file, run, problems) => {
    setGraph(graph)
    setFile(file)
    setRun(run)
    setProblems(problems ?? [])
    setActing(false)
    const text = runStatusText(run)
    if (text) setStatus({ text, error: run.outcome === 'failed' })
  }, [])

  const marks = useMemo(() => nodeMarks(problems, run), [problems, run])

  useEffect(() => {
    if (graph === null) return
    // Selection and a drag in flight are view state: the definition push
    // replaces what is drawn, never what the user has picked or holds.
    setNodes((current) => {
      const before = new Map(current.map((node) => [node.id, node]))
      return toNodes(graph, listing?.types, listing?.baseScalars, marks).map((node) => {
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
  }, [graph, listing, marks, fitView])

  useEffect(
    () =>
      connect({
        onOpen: (request) => {
          protocol.current = request
          request('list_node_types').then(setListing, showToast)
          request('get_definition').then(
            ({ graph, file, run, problems }) => resync(graph, file, run, problems),
            showToast,
          )
        },
        onGreeting: () => {
          setConnection('open')
          setStatus({ text: '' })
        },
        onVersionMismatch: (text) => {
          protocol.current = null
          setMismatch(text)
          setConnection('incompatible')
        },
        onDefinition: resync,
        onFile: ({ path, dirty }) => setFile({ path, dirty }),
        onRun: (state) => {
          setRun(state)
          setActing(false)
          setStatus({ text: runStatusText(state), error: state.outcome === 'failed' })
          // A failure is a happening: the toast carries it, and the
          // failed node's mark carries the explanation after the toast
          // is gone.
          if (state.outcome === 'failed') showToast(state.error ?? 'the run failed')
        },
        onError: showToast,
        onClosed: () => {
          protocol.current = null
          setActing(false)
          setConnection('lost')
          setStatus({ text: '', error: false })
        },
      }),
    [resync, showToast],
  )

  // The editing seam every field commit rides: one operation out, the
  // definition push re-rendering both views. An error — a commit for a
  // since-connected input, say — arrives through the toast surface, the
  // definition untouched.
  const edit = useCallback((type, fields) => {
    protocol.current?.(type, fields)?.catch(showToast)
  }, [showToast])

  const onNodesChange = useCallback((changes) => {
    // A remove change is the server's to apply: the delete operation goes
    // out, the definition push takes the node and its wires off the
    // canvas, and the view never holds a deletion the server refused.
    for (const change of changes) {
      if (change.type === 'remove' && protocol.current !== null) {
        protocol.current('delete_node', { uuid: change.id }).catch(showToast)
      }
    }
    setNodes((current) =>
      applyNodeChanges(
        changes.filter((change) => change.type !== 'remove'),
        current,
      ),
    )
  }, [showToast])

  const onNodeDragStop = useCallback((_event, node) => {
    if (node.type === 'placeholder' || protocol.current === null) return
    protocol.current('move_node', {
      uuid: node.id,
      position: { x: node.position.x, y: node.position.y },
    }).catch(showToast)
  }, [showToast])

  const onConnect = useCallback((connection) => {
    if (protocol.current === null) return
    protocol.current('wire', {
      from: connection.source,
      from_port: connection.sourceHandle,
      to: connection.target,
      to_port: connection.targetHandle,
    }).catch(showToast)
  }, [showToast])

  const onConnectEnd = useCallback(
    (_event, state) => {
      const unhook = offPortUnhook(graph?.edges ?? [], state)
      if (unhook !== null && protocol.current !== null) {
        protocol.current('unhook', unhook).catch(showToast)
      }
    },
    [graph, showToast],
  )

  const onDrop = useCallback(
    (event) => {
      event.preventDefault()
      const typeRef = event.dataTransfer.getData(NODE_TYPE)
      if (typeRef === '' || !editable || protocol.current === null) return
      const position = screenToFlowPosition({ x: event.clientX, y: event.clientY })
      protocol
        .current('create_node', { type_ref: typeRef, position })
        .catch(showToast)
    },
    [editable, screenToFlowPosition, showToast],
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
      showToast(error)
    })
  }, [confirmDiscard, showToast])

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
        showToast(error)
      })
  }, [confirmDiscard, file, showToast])

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
        protocol.current('save_file', { path }).catch(showToast)
      } else {
        protocol.current('save_file').catch(showToast)
      }
    },
    [file, showToast],
  )

  // The run control's one act: a start hands the held definition to the
  // compiler and runs it, a stop ends the run that is on, and the
  // run-state push flips the control and the lock either way. Between
  // the click and that landing further clicks are ignored — the guard
  // above — so the second click of a double-click cannot land on the
  // not-yet-flipped label and end the run the first click began.
  const actOnRun = useCallback(() => {
    if (acting || protocol.current === null) return
    setActing(true)
    protocol
      .current(run?.running === true ? 'stop_run' : 'start_run')
      .catch((error) => {
        setActing(false)
        showToast(error)
      })
  }, [acting, run, showToast])

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
  const control = runControl(run, graph, connected)
  const banner = bannerText(connection, mismatch)

  return (
    <EditContext.Provider value={edit}>
      <LockContext.Provider value={!editable}>
        <div className={editable ? 'app' : 'app locked'}>
          <header className="chrome">
            <span className="file-name">{file?.path ?? 'untitled'}</span>
            {hasUnsavedChanges(file) && (
              <span className="file-dirty">unsaved changes</span>
            )}
            <span className="chrome-space" />
            <button disabled={!control.enabled} onClick={actOnRun}>
              {control.label}
            </button>
            <button disabled={!editable} onClick={newGraph}>New</button>
            <button disabled={!editable} onClick={openFile}>Open</button>
            <button disabled={!editable} onClick={() => save(false)}>Save</button>
            <button disabled={!editable} onClick={() => save(true)}>Save as</button>
          </header>
          {/* The banner names the loss without covering the canvas: the
              view beneath stays where the user left it, looking live. */}
          {banner && <div className="banner">{banner}</div>}
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
                      draggable={editable}
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
                // Delete/Backspace is the deletion gesture — the lock takes
                // it away while a run is on, as while the connection is
                // gone. A wire is a drag from either end — a click never
                // starts or lands one — and the drag threshold keeps a
                // port click from reading as a drag-off.
                deleteKeyCode={editable ? ['Delete', 'Backspace'] : null}
                nodesDraggable={editable}
                nodesConnectable={editable}
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
              <div className="toasts">
                {toasts.map((toast) => (
                  <div
                    key={toast.id}
                    className="toast"
                    role="status"
                    onClick={() => dismissToast(toast.id)}
                  >
                    {toast.text}
                  </div>
                ))}
              </div>
            </main>
            <Sidebar
              node={selected}
              type={listing?.types.find((type) => type.type_ref === selected?.type_ref)}
              wiredInputs={selectedWiredInputs}
              baseScalars={listing?.baseScalars ?? {}}
            />
          </div>
          <footer
            className={`status${status.error ? ' error' : ''}`}
            aria-live="polite"
          >
            {status.text}
          </footer>
        </div>
      </LockContext.Provider>
    </EditContext.Provider>
  )
}
