// The editor shell: a react-flow style canvas under light Blueprint
// chrome, with the palette of every linked plugin's node types docked on
// the left and the editing sidebar docked on the right while a selection
// is open. The browser holds no authoritative graph state — it requests
// the listing and the definition on load, renders what the server holds,
// and every edit lands server-side and is pushed back. Drags follow the
// pointer locally; one operation commits where a dragged node rests,
// where a wire lands, which node a key press deletes, and what a field
// commit holds — the sidebar and the node's inline fields both committing
// through the same seam, both views of one stored value.
//
// The selection is view state over those gestures, the way scroll
// position is: a shift-click-drag on the background marquees, a
// shift-click toggles a node in or out, plain clicks keep their one-node
// meaning. What the selection does fans out to the existing per-node
// operations — a multi-select move the existing move once per node, a
// multi-select delete the existing delete once per node, a common-field
// commit the existing parameter edit once per node — so the server
// receives no message it did not already answer.
//
// While a run is on, the canvas animates from the server's pushes: node
// statuses arrive already derived — pushed as state, never recomputed
// here — and each forwarded emission animates the wires it travels and
// replaces the text at its emitting port. Emissions coalesce through one
// animation frame (the coalescer, coalesce.js): a fast graph updates once
// per frame, dropping frames, never queueing a backlog, and only the
// latest value per port is kept — what the canvas shows is always the
// events' own. A new run resets the canvas; the last run's statuses and
// values persist after it ends, until the next start.
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
  SelectionMode,
  applyNodeChanges,
  useReactFlow,
} from '@xyflow/react'
import { NODE_TYPE, connect, connectionLost } from './protocol.js'
import { RunCoalescer } from './coalesce.js'
import { EditContext, LockContext, scalarPossible } from './fields.jsx'
import {
  PluginUiContext,
  bodyTypeRefs,
  createBundleStore,
  valueTypeRefs,
} from './pluginui.jsx'
import { createElement } from 'react'
import { SelfLoopEdge } from './edges.jsx'
import { byName, portAppearance } from './types.js'
import { PlaceholderNode, TypeIcon, TypeNode } from './nodes.jsx'
import { Sidebar, SelectionSidebar } from './sidebar.jsx'

const nodeTypes = { type: TypeNode, placeholder: PlaceholderNode }
const edgeTypes = { selfloop: SelfLoopEdge }

// The fixed grid a node without a recorded position lands on, ordered by
// uuid: a deterministic, view-local fallback every view and reload agrees
// on, with nothing written into the definition.
const FALLBACK_PITCH = { x: 180, y: 140 }

// The palette's sections, from the listing's plugin and sub-group facts:
// one section per plugin, a headed sub-section per declared sub-group
// within it, the ungrouped types directly under the plugin header.
// Plugins, sub-groups, and types order alphabetically, a type's label tie
// broken by its type reference — a deterministic order every tab and
// reload agrees on.
export function paletteGroups(types) {
  const plugins = new Map()
  for (const type of types) {
    const subGroup = type.sub_group ?? null
    const sections = plugins.get(type.plugin) ?? new Map()
    sections.set(subGroup, [...(sections.get(subGroup) ?? []), type])
    plugins.set(type.plugin, sections)
  }
  return [...plugins.keys()].sort(byName).map((plugin) => {
    const sections = plugins.get(plugin)
    return {
      plugin,
      sections: [...sections.keys()]
        .sort((a, b) => (a === null ? -1 : b === null ? 1 : byName(a, b)))
        .map((subGroup) => ({
          subGroup,
          types: sections
            .get(subGroup)
            .sort((a, b) => byName(a.label, b.label) || byName(a.type_ref, b.type_ref)),
        })),
    }
  })
}

function recordedPosition(node) {
  const position = node.metadata?.position
  if (typeof position?.x === 'number' && typeof position?.y === 'number') {
    return { x: position.x, y: position.y }
  }
  return undefined
}

export function toNodes(graph, listing, marks, statuses, values) {
  const byRef = new Map((listing?.types ?? []).map((type) => [type.type_ref, type]))
  const dataTypes = listing?.dataTypes ?? {}
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
  const status = (uuid) => statuses?.[uuid]
  const value = (uuid, port) => values?.[`${uuid}/${port}`]
  return graph.nodes.map((node) => {
    const position = recordedPosition(node) ?? fallback.get(node.uuid)
    const type = byRef.get(node.type_ref)
    if (type === undefined) {
      return {
        id: node.uuid,
        position,
        type: 'placeholder',
        data: { label: node.label ?? node.type_ref, marks: at(node.uuid), status: status(node.uuid) },
        selectable: true,
      }
    }
    const portValues = {}
    for (const port of type.outputs) {
      const text = value(node.uuid, port.name)
      if (text !== undefined) portValues[port.name] = text
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
          .filter((port) => scalarPossible(port, listing?.baseScalars))
          .map((port) => port.name),
        marks: at(node.uuid),
        status: status(node.uuid),
        portValues,
        dataTypes,
      },
    }
  })
}

// The definition's edges as canvas wires. Not selectable: drag-off is the
// one way a wire comes off, so there is no second, selected-then-deleted
// path. A wire renders in the colour of the source port's declared type —
// the neutral where no single type informs the port; a wire in `pulsing`
// animates, the pulse a value travels riding that colour as a dash flow.
export function toEdges(graph, pulsing, listing) {
  const byRef = new Map((listing?.types ?? []).map((type) => [type.type_ref, type]))
  const dataTypes = listing?.dataTypes ?? {}
  const refOf = new Map(graph.nodes.map((node) => [node.uuid, node.type_ref]))
  return graph.edges.map((edge) => {
    const id = `${edge.from}/${edge.from_port}->${edge.to}/${edge.to_port}`
    const type = byRef.get(refOf.get(edge.from))
    const port = type?.outputs.find((output) => output.name === edge.from_port)
    return {
      id,
      source: edge.from,
      sourceHandle: edge.from_port,
      target: edge.to,
      targetHandle: edge.to_port,
      selectable: false,
      animated: pulsing?.has(id) === true,
      type: edge.from === edge.to ? 'selfloop' : undefined,
      style: { stroke: portAppearance(port, dataTypes).color },
    }
  })
}

// The wires one emission animates: every downstream wire of the emitting
// port — a fan-out pulses each path the value actually takes.
export function emittedTargets(edges, node, port) {
  return edges
    .filter((edge) => edge.from === node && edge.from_port === port)
    .map((edge) => `${edge.from}/${edge.from_port}->${edge.to}/${edge.to_port}`)
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

// Whether a keyboard event's target sits in a text field: the keys keep
// their editing meaning there, so the canvas's own — delete among them —
// wait for focus to return.
export function inTextField(target) {
  return target?.closest?.('input, textarea, select, [contenteditable]') != null
}

// The moves one drag stop commits — the existing move operation, one per
// node that travelled. A multi-selection moves whole: every node that
// travelled, placeholders included, the operation being uuid-addressed.
// A selection of one keeps story 12's line: a typed node commits itself.
export function dragMoves(group) {
  return group.map((node) => ({ uuid: node.id, position: node.position }))
}

// Which nodes the delete key removes: a multi-selection takes every
// selected node, placeholders included — uuid-addressed operations need
// no type knowledge — while a selection of one is story 12's delete, a
// typed node spared nothing and a lone placeholder spared.
export function deleteSelection(nodes) {
  const selected = nodes.filter((node) => node.selected)
  if (selected.length === 1 && selected[0].type === 'placeholder') return []
  return selected.map((node) => node.id)
}

// Which nodes a Delete/Backspace keystroke removes, or none — the whole
// guard chain in one place. The gesture lives on the canvas in focus: a
// text field keeps the keys as text editing, and so does focus anywhere
// else in the chrome, while no focused element at all reads as the
// canvas's — the canvas takes no focus of its own. The lock takes the
// gesture away with every other edit, and a held key repeats nothing.
export function deleteKeys(event, editable, nodes, canvas) {
  if (!editable || event.repeat) return []
  if (event.key !== 'Delete' && event.key !== 'Backspace') return []
  if (inTextField(event.target)) return []
  if (!canvas?.contains(event.target) && event.target !== canvas?.ownerDocument?.body) {
    return []
  }
  return deleteSelection(nodes)
}

// A placeholder takes part in the uuid-addressed gestures exactly when it
// is selected inside a multi-selection and editing is unlocked: story 12
// keeps a lone placeholder inert, a placeholder outside the selection
// never travels on another pair's account, and the lock holds it as it
// holds every node. This is the draggable flag the canvas drags read,
// set from the selection wherever selection changes.
export function withDraggablePlaceholders(nodes, editable) {
  const multi = nodes.filter((node) => node.selected).length > 1
  return nodes.map((node) =>
    node.type === 'placeholder'
      ? { ...node, draggable: multi && editable && node.selected }
      : node,
  )
}

// The background's two drags differ by Shift alone, and so does their
// catch: a plain drag pans, a shift-drag draws the marquee, and the
// marquee takes every node the rectangle touches — inside or
// intersecting, the catch a sweep across a clump expects. The props the
// canvas reads for all of it.
export function backgroundDrag(shift) {
  return {
    selectionOnDrag: shift,
    panOnDrag: !shift,
    selectionMode: SelectionMode.Partial,
  }
}

// The node changes the canvas applies, save removal: a remove change
// would drop nodes the server still holds, with no operation sent and no
// refusal path — deletion is the shell's own gesture, fanned to the
// delete operation and applied by the definition push that answers it.
export function viewNodeChanges(changes, nodes) {
  return applyNodeChanges(
    changes.filter((change) => change.type !== 'remove'),
    nodes,
  )
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
// reach the server, so a connection that cannot deliver them is the
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

// The banner a connection state names, or none: the loss names itself and
// the reconnection the client is already making; the mismatch names itself
// and the reload that is the advice — never a self-dismissing toast. A
// connected or not-yet-lost editor has no banner.
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
  // The canvas's view of the run, exactly what the bridge holds: the
  // derived status per node and the latest value per emitting port,
  // rendered as pushed — statuses via node_status state, values via the
  // forwarded emissions and the connect-time snapshot.
  const [statuses, setStatuses] = useState({})
  const [values, setValues] = useState({})
  const [pulsing, setPulsing] = useState(() => new Set())
  const [nodes, setNodes] = useState([])
  // The loaded plugin bundles, per attachment point: a node type's body
  // component and a type's value component, absent until the first node of
  // the type or the first displayed value asks for the bundle, and null
  // once a load or a render has failed there — the default class and the
  // contentless readout taking over, the failure reported.
  const [bodies, setBodies] = useState({})
  const [valueBodies, setValueBodies] = useState({})
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
  // The canvas's nodes as the delete gesture reads them: the key handler
  // lives across renders and reads the latest selection without
  // re-subscribing for every drag frame.
  const nodesRef = useRef([])
  useEffect(() => {
    nodesRef.current = nodes
  })
  // The held definition's edges, as the event path reads them: the
  // emission handler lives across renders, and the wires it pulses are
  // the ones the last resync carried.
  const graphRef = useRef(null)
  // The coalescer owns the canvas's view of the run's values and pulses,
  // applying each animation frame's truth to the state above; the status
  // per node needs no coalescing and stays state alone.
  const coalescer = useRef(null)
  if (coalescer.current === null) {
    coalescer.current = new RunCoalescer(({ values, pulsing }) => {
      setValues(values)
      setPulsing(pulsing)
    })
  }
  // Armed by a replacement gesture — open, new — and consumed by the
  // next definition arrival, which brings the view to the graph; a
  // failed gesture disarms it.
  const pendingRefit = useRef(false)
  // The page's one bundle table: whatever node UI and value UI the
  // listing's facts name, loaded once each however many nodes and ports
  // ask for them.
  const bundles = useRef(null)
  if (bundles.current === null) bundles.current = createBundleStore()
  const { screenToFlowPosition, getViewport, setViewport, fitView } = useReactFlow()

  // The background's two drags differ by Shift alone, and so do the two
  // node clicks: the shell tracks Shift itself and hands the canvas the
  // two modes (backgroundDrag below), so a shift-drag on the background
  // marquees while a plain drag pans, and a pointerdown on a node stays
  // the node's — to select, to toggle, to drag — rather than being
  // swallowed by the marquee. The tracker resets on window blur, so
  // Shift cannot stick past the window that held it.
  const [shiftHeld, setShiftHeld] = useState(false)
  useEffect(() => {
    const onKey = (event, down) => {
      if (event.key === 'Shift') setShiftHeld(down)
    }
    const down = (event) => onKey(event, true)
    const up = (event) => onKey(event, false)
    const blur = () => setShiftHeld(false)
    window.addEventListener('keydown', down)
    window.addEventListener('keyup', up)
    window.addEventListener('blur', blur)
    return () => {
      window.removeEventListener('keydown', down)
      window.removeEventListener('keyup', up)
      window.removeEventListener('blur', blur)
    }
  }, [])

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
  // truth, which lives in the marks and the chrome state. A connection
  // loss is chrome state of its own, the banner's: a request the loss
  // settles arrives marked `connectionLost` and is not toasted beside
  // the banner saying the same thing.
  const dismissToast = useCallback((id) => {
    setToasts((current) => current.filter((toast) => toast.id !== id))
  }, [])
  const showToast = useCallback(
    (report) => {
      if (report === connectionLost) return
      const id = toastId.current += 1
      setToasts((current) => [...current, { id, text: report }])
      setTimeout(() => dismissToast(id), TOAST_MS)
    },
    [dismissToast],
  )

  // A definition arrival — the connect-time resync or a push — replaces
  // what is drawn, and the run state it carries narrates itself the way a
  // run push does. An idle state with no outcome — what a compile failure
  // leaves — reads as '', leaving the status line, the refused start's
  // error report, as it is. The run display is not part of it: the
  // statuses and values arrive as their own push, on the ordered stream
  // the live changes ride.
  const resync = useCallback((graph, file, run, problems) => {
    graphRef.current = graph
    setGraph(graph)
    setFile(file)
    setRun(run)
    setProblems(problems ?? [])
    setActing(false)
    const text = runStatusText(run)
    if (text) setStatus({ text, error: run.outcome === 'failed' })
  }, [])

  const marks = useMemo(() => nodeMarks(problems, run), [problems, run])

  // A failed bundle is reported through the toast surface naming the type,
  // and its attachment point marked fallen-back: the node renders by the
  // default class, the value readout stays contentless, and neither asks
  // for the bundle again this page. A load in flight collects one
  // rejection handler per effect run that found it pending — the value
  // effect re-runs per animation frame while its type's values stream —
  // so the report, unlike the fallback, is guarded per attachment point:
  // one failure names the type once.
  const reportedBodies = useRef(new Set())
  const reportedValues = useRef(new Set())
  const failedBundle = (reported, setState) => (ref, error) => {
    setState((current) => (current[ref] === null ? current : { ...current, [ref]: null }))
    if (reported.current.has(ref)) return
    reported.current.add(ref)
    showToast(`plugin UI for ${ref} failed: ${error?.message ?? error}`)
  }
  const failBody = useCallback(failedBundle(reportedBodies, setBodies), [showToast])
  const failValue = useCallback(failedBundle(reportedValues, setValueBodies), [showToast])

  // Node-type UI loads lazily: the first node of the type on the canvas
  // asks for its bundle, the palette asking for nothing. Value UI loads
  // the same way, at the first displayed value. A bundle that has not
  // arrived delays nothing — the nodes it belongs to render by the default
  // class until it lands.
  useEffect(() => {
    if (graph === null || listing === null) return
    const byRef = new Map(listing.types.map((type) => [type.type_ref, type]))
    for (const ref of bodyTypeRefs(graph, listing.types)) {
      if (bodies[ref] !== undefined) continue
      bundles.current.attempt(byRef.get(ref).ui).then(
        (body) => setBodies((current) => ({ ...current, [ref]: body })),
        (error) => failBody(ref, error),
      )
    }
  }, [graph, listing, bodies, failBody])

  useEffect(() => {
    if (graph === null || listing === null) return
    for (const ref of valueTypeRefs(graph, listing.types, listing.dataTypes, values)) {
      if (valueBodies[ref] !== undefined) continue
      bundles.current.attempt(listing.dataTypes[ref].ui).then(
        (body) => setValueBodies((current) => ({ ...current, [ref]: body })),
        (error) => failValue(ref, error),
      )
    }
  }, [graph, listing, values, valueBodies, failValue])

  useEffect(() => {
    if (graph === null) return
    // Selection and a drag in flight are view state: the definition push
    // replaces what is drawn, never what the user has picked or holds.
    setNodes((current) => {
      const before = new Map(current.map((node) => [node.id, node]))
      return withDraggablePlaceholders(
        toNodes(graph, listing, marks, statuses, values).map((node) => {
          const held = before.get(node.id)
          return {
            ...node,
            selected: held?.selected ?? false,
            position: held?.dragging ? held.position : node.position,
          }
        }),
        editable,
      )
    })
    // A replacement brings the view to the graph that arrived, the way
    // the launch fit does; the queued fit waits for the new nodes to be
    // measured. An ordinary edit push leaves the view to the user.
    if (pendingRefit.current) {
      pendingRefit.current = false
      fitView({ maxZoom: 1 })
    }
  }, [graph, listing, marks, statuses, values, editable, fitView])

  useEffect(() => () => coalescer.current.dispose(), [])

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
        // The run display arrives as its own push — the connect-time
        // resync's snapshot and nothing else; the live changes that keep
        // it true ride the same ordered stream behind it.
        onRunDisplay: ({ statuses, values }) => {
          setStatuses(statuses ?? {})
          coalescer.current.replace(values ?? {})
        },
        onRun: (state) => {
          setRun(state)
          setActing(false)
          // A new run resets the canvas: the last run's statuses and
          // values give way as this run's own events arrive.
          if (state.running) {
            setStatuses({})
            coalescer.current.replace({})
          }
          setStatus({ text: runStatusText(state), error: state.outcome === 'failed' })
          // A failure is a happening: the toast carries it, and the
          // failed node's mark carries the explanation after the toast
          // is gone.
          if (state.outcome === 'failed') showToast(state.error ?? 'the run failed')
        },
        onRunEvent: (message) => {
          // The statuses and the outcome ride their own state pushes;
          // the emissions are what the canvas animates: the value at the
          // emitting port, the wires the value travels.
          if (message.event !== 'emitted') return
          coalescer.current.emitted(
            message.node,
            message.port,
            message.value,
            emittedTargets(graphRef.current?.edges ?? [], message.node, message.port),
          )
        },
        onNodeStatus: ({ node, status: derived }) =>
          setStatuses((current) => ({ ...current, [node]: derived })),
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
    setNodes((current) =>
      withDraggablePlaceholders(viewNodeChanges(changes, current), editable),
    )
  }, [editable])

  const onNodeDragStop = useCallback((_event, _node, group) => {
    if (protocol.current === null) return
    for (const { uuid, position } of dragMoves(group)) {
      protocol
        .current('move_node', { uuid, position: { x: position.x, y: position.y } })
        .catch(showToast)
    }
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

  // The delete gesture, view-side: Delete/Backspace with the canvas in
  // focus, the whole guard chain pinned in deleteKeys. The selection
  // fans out to the existing delete operation, one per node, edges
  // cascading server-side — no dangling wires.
  useEffect(() => {
    if (!editable) return
    const onKeyDown = (event) => {
      for (const uuid of deleteKeys(event, editable, nodesRef.current, canvasRef.current)) {
        protocol.current?.('delete_node', { uuid })?.catch(showToast)
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [editable, showToast])

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
          withDraggablePlaceholders(
            current.map((node) => ({ ...node, selected: false })),
            editable,
          ),
        ),
      )
      .catch((error) => {
        pendingRefit.current = false
        showToast(error)
      })
  }, [confirmDiscard, editable, file, showToast])

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

  // The sidebar's selection: every node the canvas holds selected, read
  // off the same node state the canvas draws, so a definition push swaps
  // its contents with everything else. One selected is the node's own
  // view; several are the selection's — the fields they hold in common,
  // committed to every node through the per-node operation each one is.
  const selectedNodes =
    graph?.nodes.filter((node) =>
      nodes.some((drawn) => drawn.selected && drawn.id === node.uuid),
    ) ?? []
  const selected = selectedNodes.length === 1 ? selectedNodes[0] : undefined
  const selectedUuid = selectedNodes.length === 1 ? selected.uuid : null
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
                paletteGroups(types).map((group) => (
                  <section className="palette-plugin" key={group.plugin}>
                    <h2 className="palette-heading">{group.plugin}</h2>
                    {group.sections.map((section) => (
                      <div className="palette-group" key={section.subGroup ?? ''}>
                        {section.subGroup !== null && (
                          <h3 className="palette-subgroup">{section.subGroup}</h3>
                        )}
                        <ul className="palette-list">
                          {section.types.map((type) => (
                            <li
                              key={type.type_ref}
                              className="palette-item"
                              draggable={editable}
                              onDragStart={(event) => {
                                event.dataTransfer.setData(NODE_TYPE, type.type_ref)
                                event.dataTransfer.effectAllowed = 'move'
                              }}
                            >
                              <TypeIcon icon={type.icon} />
                              <span className="palette-label">{type.label}</span>
                            </li>
                          ))}
                        </ul>
                      </div>
                    ))}
                  </section>
                ))
              )}
            </aside>
            <main className="canvas" ref={canvasRef}>
              {/* The plugin UI contract rides the canvas subtree alone: its
                  readers are the node renderings, nothing in the chrome. */}
              <PluginUiContext.Provider
                value={{ h: createElement, bodies, valueBodies, failBody, failValue }}
              >
                <ReactFlow
                  nodes={nodes}
                  edges={graph === null ? [] : toEdges(graph, pulsing, listing)}
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
                  // Shift rides the shell's own tracker — backgroundDrag
                  // above — and multiSelectionKeyCode makes a shift-click
                  // on a node a toggle, React Flow's selection key retired
                  // so its capture can never swallow a node's pointerdown.
                  // The delete key is this shell's own gesture — fanning
                  // out to the delete operation per selected node — so
                  // React Flow's is retired. A wire is a drag from either
                  // end — a click never starts or lands one — and the drag
                  // threshold keeps a port click from reading as a drag-off.
                  multiSelectionKeyCode="Shift"
                  selectionKeyCode={null}
                  deleteKeyCode={null}
                  {...backgroundDrag(shiftHeld)}
                  nodesDraggable={editable}
                  nodesConnectable={editable}
                  connectionDragThreshold={4}
                  connectOnClick={false}
                >
                  <Background variant="dots" gap={24} size={1.5} />
                </ReactFlow>
              </PluginUiContext.Provider>
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
            {selectedNodes.length > 1 && (
              <SelectionSidebar
                nodes={selectedNodes}
                types={new Map((listing?.types ?? []).map((type) => [type.type_ref, type]))}
                baseScalars={listing?.baseScalars ?? {}}
                edges={graph?.edges ?? []}
              />
            )}
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
