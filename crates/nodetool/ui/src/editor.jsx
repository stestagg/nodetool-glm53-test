// The editor shell: a react-flow style canvas under light Blueprint
// chrome, with the palette of every linked plugin's node types docked on
// the left and the editing sidebar docked on the right while a selection
// is open. The browser holds no authoritative graph state — it requests
// the listing and the definition on load, renders what the server holds,
// and every edit lands server-side and is pushed back. Drags follow the
// pointer locally; one operation commits where a dragged node rests,
// where a wire lands, which node a key press deletes, and what a field
// commit holds — the sidebar and the node's inline fields both committing
// through the same seam, both views of one stored value. The wire
// (wire.js) holds the connection and everything it delivers.
//
// The selection is view state over those gestures, the way scroll
// position is: a shift-click-drag on the background marquees, a
// shift-click toggles a node in or out, plain clicks keep their one-node
// meaning, and the keyboard mirrors each — Enter or Space selects the
// focused node, Shift with them toggles it in the selection, Escape
// clears. What the selection does fans out to the existing per-node
// operations — a multi-select move the existing move once per node, a
// multi-select delete the existing delete once per node, a common-field
// commit the existing parameter edit once per node — so the server
// receives no message it did not already answer.
//
// The keyboard reaches every editing gesture the pointer does, through
// the same operations: a palette row's activation creates where a drop
// does, at the view's centre; a focused node's arrow keys nudge the
// selection through the same move a drag stop commits; a focused port
// starts, lands, and unhooks wires through the same wire and unhook
// operations, Escape standing the wire down; Delete keeps its
// canvas-focus scoping. Focus is the keyboard's way around the canvas —
// pan and zoom stay pointer-only, the view following focus instead.
// keyboard.js holds the decisions; the canvas gestures sit on React
// Flow's own interaction primitives, adjusted, not replaced.
//
// Every report arrives through one surface: a toast in the chrome,
// dismissed on click and on its own. What toasts deliberately do not
// carry is the durable truth — a problem a compile found sits as a mark
// on the node it names, a failed run's explanation sits on the failed
// node, and both are gone when the problem or the next run is. A lost
// connection is chrome state of its own: a banner naming the loss over
// the last-known canvas, every server-acting gesture inert while the
// looking stays live, the tab rejoining by itself when the server
// returns. Both reports reach assistive technology as they appear, the
// polite live regions they arrive through being chrome.jsx's.

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Background, ReactFlow, useReactFlow } from '@xyflow/react'
import { NODE_TYPE, connectionLost } from './protocol.js'
import { EditContext, LockContext, WireContext } from './fields.jsx'
import {
  PluginUiContext,
  bodyTypeRefs,
  createBundleStore,
  valueTypeRefs,
} from './pluginui.jsx'
import { createElement } from 'react'
import { SelfLoopEdge } from './edges.jsx'
import { PlaceholderNode, TypeNode } from './nodes.jsx'
import { Sidebar, SelectionSidebar } from './sidebar.jsx'
import { Banner, Toasts } from './chrome.jsx'
import { ContextMenu } from './menu.jsx'
import { Palette } from './palette.jsx'
import {
  backgroundMenuKeys,
  escapeCancel,
  finalMoves,
  groupKeys,
  panIntoView,
  toggleKey,
  viewportCenter,
} from './keyboard.js'
import { groupInstances, groupNames, groupSuggestion, groupType } from './groups.js'
import { useEditorWire } from './wire.js'
import {
  backgroundDrag,
  bannerText,
  carriedNode,
  deleteKeys,
  hasUnsavedChanges,
  nodeMarks,
  offPortUnhook,
  runControls,
  saveAsksForPath,
  toEdges,
  toNodes,
  toggledSelection,
  viewNodeChanges,
  withDraggablePlaceholders,
} from './view.js'

const nodeTypes = { type: TypeNode, placeholder: PlaceholderNode }
const edgeTypes = { selfloop: SelfLoopEdge }

// How long a toast stays when nobody dismisses it.
const TOAST_MS = 8000

export function Editor() {
  const [nodes, setNodes] = useState([])
  // The loaded plugin bundles, per attachment point: a node type's body
  // component and a type's value component, absent until the first node of
  // the type or the first displayed value asks for the bundle, and null
  // once a load or a render has failed there — the default class and the
  // contentless readout taking over, the failure reported.
  const [bodies, setBodies] = useState({})
  const [valueBodies, setValueBodies] = useState({})
  const [toasts, setToasts] = useState([])
  // The keyboard wire in progress: the port it runs from, or null. The
  // ports read it for their origin handle; Escape and a landing stand it
  // down, and the lock takes it away as it takes every edit.
  const [wire, setWire] = useState(null)
  const canvasRef = useRef(null)
  const toastId = useRef(0)
  // The canvas's nodes as the gesture handlers read them: the key
  // handlers live across renders and read the latest selection without
  // re-subscribing for every drag frame.
  const nodesRef = useRef([])
  useEffect(() => {
    nodesRef.current = nodes
  })
  // Armed by a replacement gesture — open, new — and consumed by the
  // next definition arrival, which brings the view to the graph; a
  // failed gesture disarms it.
  const pendingRefit = useRef(false)
  // The selection a finished packaging gesture hands its product: the
  // uuids the reply named, taken once the definition push carrying them
  // has landed — reply and push race on one socket, and until the nodes
  // exist the handoff waits.
  const [handoff, setHandoff] = useState(null)
  // The context menu's seat, in canvas coordinates, or null, with the
  // flow position its create lands at, the selection it opened for and
  // whether it is the background's own: the selection's gestures where
  // the pointer is, the background's Groups beside them, the entries
  // re-derived from the held definition every render.
  const [menu, setMenu] = useState(null)
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

  // The wire: the connection and everything it delivers, held in one
  // place — the listing and the definition, the run and its display, the
  // status line and the connection's states — with the run control's act.
  const {
    listing,
    graph,
    file,
    run,
    problems,
    statuses,
    values,
    pulsing,
    connection,
    mismatch,
    status,
    protocol,
    graphRef,
    factsRef,
    actOnRun,
  } = useEditorWire(showToast)

  const connected = connection === 'open'
  // While a run is on the definition is held still, and while the
  // connection is gone nothing can reach the server: every editing
  // gesture goes quiet — palette drops and activations, moves, wires,
  // unhooking, deletion, label and parameter edits, and the file
  // controls — while selection, panning, and zoom stay live, looking not
  // being editing. The lock is one rule over both input modes: the
  // pointer gesture and its keyboard twin quiet together. The server
  // refuses whatever slips through when the lock is the reason; when the
  // connection is, nothing is sent at all — the browser holds no graph
  // state that could back an undeliverable edit.
  const locked = run?.running === true
  const editable = connected && !locked

  const marks = useMemo(() => nodeMarks(problems, run, factsRef.current?.owner), [problems, run])

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
    // Selection, a drag in flight, and the measurement the canvas took
    // are view state: the definition push replaces what is drawn, never
    // what the user has picked or holds (carriedNode). A pending handoff
    // replaces the carried selection once the nodes it names exist — the
    // product of a package or an unpack taking the selection.
    const handed =
      handoff !== null && handoff.every((uuid) => graph.nodes.some((node) => node.uuid === uuid))
    setNodes((current) => {
      const before = new Map(current.map((node) => [node.id, node]))
      const rebuilt = withDraggablePlaceholders(
        toNodes(graph, listing, marks, statuses, values, factsRef.current).map((node) =>
          carriedNode(node, before.get(node.id)),
        ),
        editable,
      )
      if (!handed) return rebuilt
      return rebuilt.map((node) => ({ ...node, selected: handoff.includes(node.id) }))
    })
    if (handed) setHandoff(null)
    // A replacement brings the view to the graph that arrived, the way
    // the launch fit does; the queued fit waits for the new nodes to be
    // measured. An ordinary edit push leaves the view to the user.
    if (pendingRefit.current) {
      pendingRefit.current = false
      fitView({ maxZoom: 1 })
    }
  }, [graph, listing, marks, statuses, values, editable, handoff, fitView])

  // The editing seam every field commit rides: one operation out, the
  // definition push re-rendering both views. An error — a commit for a
  // since-connected input, say — arrives through the toast surface, the
  // definition untouched.
  const edit = useCallback((type, fields) => {
    protocol.current?.(type, fields)?.catch(showToast)
  }, [showToast])

  // The node changes the canvas applies, and the moves they commit: a
  // drag's travel frames are view only, the rests — a drag stop, a
  // keyboard nudge — each committing the move operation its gesture
  // lands on, one per node, the way the drag stop always did.
  const onNodesChange = useCallback(
    (changes) => {
      setNodes((current) =>
        withDraggablePlaceholders(viewNodeChanges(changes, current), editable),
      )
      const known = new Set(nodesRef.current.map((node) => node.id))
      for (const move of finalMoves(changes)) {
        if (known.has(move.uuid)) edit('move_node', move)
      }
    },
    [edit, editable],
  )

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

  // The two selection keys the shell takes before the canvas's own see
  // them, capture-side: the shift-activation, the shift-click's twin,
  // and Escape, the quiet cancel — the selection clears, the sidebar
  // closing by its rule, an in-progress keyboard wire standing down. A
  // decided cancel stops the key as the toggle does, the canvas's own
  // Escape handler on a selected node scheduling a blur of the wrapper —
  // a cancel must leave the user where they stood. Selection is view
  // state: it stays live through the lock, exactly as clicking.
  useEffect(() => {
    const onKeyDown = (event) => {
      // While the menu is up, the canvas's keys stand down — the menu is
      // the gesture in front; its Escape closes it quietly, the selection
      // standing.
      if (menu !== null) return
      const uuid = toggleKey(event)
      if (uuid !== null) {
        event.preventDefault()
        event.stopPropagation()
        setNodes((current) =>
          withDraggablePlaceholders(toggledSelection(current, uuid), editable),
        )
        return
      }
      const cancel = escapeCancel(event, nodesRef.current)
      if (cancel === null) return
      event.preventDefault()
      event.stopPropagation()
      setWire(cancel.wire)
      setNodes(withDraggablePlaceholders(cancel.nodes, editable))
    }
    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [editable, menu])

  // The lock takes an in-flight keyboard wire away with the edits.
  useEffect(() => {
    if (!editable) setWire(null)
  }, [editable])

  // The view following focus onto the canvas: React Flow pans a focused
  // node into view; a focused port is panned back by the shortest shift
  // that shows it.
  useEffect(() => {
    const canvas = canvasRef.current
    const onFocusIn = (event) => {
      const target = event.target
      if (!target?.classList?.contains('react-flow__handle')) return
      const view = panIntoView(
        getViewport(),
        canvas.getBoundingClientRect(),
        target.getBoundingClientRect(),
      )
      if (view !== null) setViewport(view)
    }
    canvas.addEventListener('focusin', onFocusIn)
    return () => canvas.removeEventListener('focusin', onFocusIn)
  }, [getViewport, setViewport])

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

  // The palette row's activation: the create a drop sends, at the view's
  // centre — a deterministic, visible position, the keyboard's answer to
  // "where it dropped".
  const createFromPalette = useCallback(
    (typeRef) => {
      if (!editable || protocol.current === null) return
      const canvas = canvasRef.current
      if (canvas === null) return
      const rect = canvas.getBoundingClientRect()
      const position = viewportCenter(getViewport(), { width: rect.width, height: rect.height })
      protocol
        .current('create_node', { type_ref: typeRef, position })
        .catch(showToast)
    },
    [editable, getViewport, showToast],
  )

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

  // The packaging gestures: the menu entries and the chords issue the
  // same operations. Unpack is the per-instance operation, once per
  // group instance in the selection, 20's fan-out.
  //
  // The selection as the held definition holds it: the operations and
  // the menu's entries address definition nodes, not the drawn ones.
  const selectedHeld = useCallback(
    () =>
      (graphRef.current?.nodes ?? []).filter((node) =>
        nodesRef.current.some((drawn) => drawn.selected && drawn.id === node.uuid),
      ),
    [],
  )

  const packageSelection = useCallback(() => {
    if (!editable || protocol.current === null) return
    const selected = selectedHeld().map((node) => node.uuid)
    if (selected.length === 0) return
    const name = window.prompt('Package into group…', groupSuggestion(graphRef.current?.groups))
    if (name === null || name === '') return
    protocol.current('package_group', { nodes: selected, name }).then(
      (reply) => setHandoff([reply.uuid]),
      showToast,
    )
  }, [editable, selectedHeld, showToast])

  const unpackSelection = useCallback(() => {
    if (!editable || protocol.current === null) return
    const instances = groupInstances(selectedHeld(), graphRef.current?.groups)
    if (instances.length === 0) return
    // One operation per instance, each refusal toasted; a refused or
    // dropped unpack contributes nothing, so the handoff carries only
    // nodes that returned — an all-refused fan-out hands over nothing.
    Promise.all(
      instances.map((uuid) =>
        protocol.current('unpack_group', { uuid }).then(
          (reply) => reply.nodes,
          (error) => {
            showToast(error)
            return []
          },
        ),
      ),
    ).then((groups) => {
      const nodes = groups.flat()
      if (nodes.length > 0) setHandoff(nodes)
    })
  }, [editable, selectedHeld, showToast])

  // The Groups submenu's pick: one instance of the named group, landing
  // where the menu was opened — the flow position carried from the open,
  // so a view change between open and pick cannot move the landing. The
  // same create a palette drop sends, position included.
  const createGroupInstance = useCallback(
    (name, position) => {
      if (!editable || protocol.current === null) return
      protocol.current('create_node', { type_ref: name, position }).catch(showToast)
    },
    [editable, showToast],
  )

  // The menu's entries, read off the held definition and the captured
  // selection at the moment they are asked — the open's decision and
  // every render after, so the Groups submenu tracks the definition live:
  // a package puts its group in it, the unpack of the last instance takes
  // it back out. A submenu with no rows is no entry; a menu with no
  // entries is no menu — the open decides by the same derivation.
  const menuEntries = useCallback(
    (acted, background, position) => {
      const entries = []
      if (acted.length > 0) {
        entries.push({ key: 'package', label: 'Package into group…', onPick: packageSelection })
      }
      if (groupInstances(acted, graphRef.current?.groups).length > 0) {
        entries.push({ key: 'unpack', label: 'Unpack group', onPick: unpackSelection })
      }
      const names = background ? groupNames(graphRef.current?.groups) : []
      if (names.length > 0) {
        entries.push({
          key: 'groups',
          label: 'Groups',
          items: names.map((name) => ({
            key: name,
            label: name,
            onPick: () => createGroupInstance(name, position),
          })),
        })
      }
      return entries
    },
    [packageSelection, unpackSelection, createGroupInstance],
  )

  // The menu's opening moves: a right-click never disturbs the selection
  // it acts on — a node already selected keeps the selection whole, an
  // unselected one is selected alone first, as a plain click does — and
  // the menu itself is an editing gesture, quiet while a run is on,
  // selection staying live beneath the lock. The background's own menu
  // carries the Groups submenu beside the selection's entries; with
  // nothing to act on and no groups it does not open — a menu whose one
  // row cannot act is noise. A right-click with nothing to act on opens
  // nothing: the open and every render after read the same derivation, so
  // `menu` set is always a menu the render shows and the closers can close.
  const openMenu = useCallback(
    (seat, position, acted, background = false) => {
      if (!editable) return
      const entries = menuEntries(acted, background, position)
      if (entries.length === 0) return
      setMenu({ x: seat.x, y: seat.y, position, acted, background })
    },
    [editable, menuEntries],
  )

  const openMenuAt = useCallback(
    (event, acted, background = false) => {
      if (canvasRef.current === null) return
      const rect = canvasRef.current.getBoundingClientRect()
      const seat = { x: event.clientX - rect.left, y: event.clientY - rect.top }
      openMenu(seat, screenToFlowPosition({ x: event.clientX, y: event.clientY }), acted, background)
    },
    [openMenu, screenToFlowPosition],
  )

  // The background menu's keyboard way in: the chord seats it at the
  // view's centre — the deterministic, visible position the palette row
  // creates at — and the create a row's activation sends lands at the
  // flow point that centre names.
  const openBackgroundMenu = useCallback(() => {
    const canvas = canvasRef.current
    if (canvas === null) return
    const rect = canvas.getBoundingClientRect()
    openMenu(
      { x: rect.width / 2, y: rect.height / 2 },
      viewportCenter(getViewport(), { width: rect.width, height: rect.height }),
      selectedHeld(),
      true,
    )
  }, [getViewport, openMenu, selectedHeld])

  // The standing menu's entries: the same derivation the open made, read
  // again at every render — the Groups submenu tracking the held
  // definition live. A derivation that comes back empty closes the menu
  // before it paints, a menu with no entries being no menu at the open
  // and after: the last group unpacked in another tab takes the standing
  // background menu with it.
  const entries = menu === null ? [] : menuEntries(menu.acted, menu.background, menu.position)
  useLayoutEffect(() => {
    if (menu !== null && entries.length === 0) setMenu(null)
  }, [menu, entries])

  const onNodeContextMenu = useCallback(
    (event, node) => {
      event.preventDefault()
      const already = nodesRef.current.some((drawn) => drawn.id === node.id && drawn.selected)
      if (!already) {
        setNodes((current) =>
          withDraggablePlaceholders(
            current.map((drawn) => ({ ...drawn, selected: drawn.id === node.id })),
            editable,
          ),
        )
      }
      // The selection the menu acts on: the whole one if the node was
      // already in it, the node alone otherwise — the same selection the
      // set above writes.
      const acted = already
        ? selectedHeld()
        : (graphRef.current?.nodes ?? []).filter((held) => held.uuid === node.id)
      openMenuAt(event, acted)
    },
    [editable, openMenuAt, selectedHeld],
  )

  const onPaneContextMenu = useCallback(
    (event) => {
      event.preventDefault()
      // The background's own menu: the selection's entries when one
      // stands, and the document's Groups — the one surface a group
      // definition reaches the canvas by.
      openMenuAt(event, selectedHeld(), true)
    },
    [openMenuAt, selectedHeld],
  )

  // The chords: Ctrl+G packages the selection, Ctrl+Shift+G unpacks its
  // group instances, and Ctrl+I opens the background menu seated at the
  // view's centre — the decisions in keyboard.js, the operations here.
  // The selection handed over is the held definition's, the shape
  // groupKeys's unpack branch reads. With the menu open the menu is the
  // gesture in front: the chords stand down with the canvas's own keys.
  useEffect(() => {
    const onKeyDown = (event) => {
      if (menu !== null) return
      if (backgroundMenuKeys(event, editable, graphRef.current?.groups)) {
        event.preventDefault()
        openBackgroundMenu()
        return
      }
      const gesture = groupKeys(event, editable, selectedHeld(), graphRef.current?.groups)
      if (gesture === null) return
      event.preventDefault()
      if (gesture === 'package') packageSelection()
      else unpackSelection()
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [editable, menu, openBackgroundMenu, packageSelection, unpackSelection, selectedHeld])

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
  // The held definition's group facts, composed once per definition
  // arrival (factsRef): a group instance's type reference resolves
  // against these before the listing's — the same resolution the canvas
  // draws by, the sidebar's single-selection view included.
  const facts = factsRef.current
  const selectedType =
    selected === undefined
      ? undefined
      : facts?.types.has(selected.type_ref)
        ? groupType(selected, facts.types.get(selected.type_ref))
        : listing?.types.find((type) => type.type_ref === selected.type_ref)
  const selectionTypes = new Map()
  for (const node of graph?.nodes ?? []) {
    const group = facts?.types.get(node.type_ref)
    if (group !== undefined && !selectionTypes.has(node.type_ref)) {
      selectionTypes.set(node.type_ref, groupType(node, group))
    }
  }
  for (const type of listing?.types ?? []) {
    if (!selectionTypes.has(type.type_ref)) selectionTypes.set(type.type_ref, type)
  }

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

  const empty =
    graph !== null &&
    graph.nodes.length === 0 &&
    listing?.types !== undefined &&
    listing.types.length > 0
  const controls = runControls(run, graph, connected)
  const banner = bannerText(connection, mismatch)

  return (
    <EditContext.Provider value={edit}>
      <LockContext.Provider value={!editable}>
        <WireContext.Provider value={{ wire, setWire }}>
          <div className={editable ? 'app' : 'app locked'}>
            <header className="chrome">
              <span className="file-name">{file?.path ?? 'untitled'}</span>
              {hasUnsavedChanges(file) && (
                <span className="file-dirty">unsaved changes</span>
              )}
              <span className="chrome-space" />
              {/* Two groups, two kinds of act: what tends the document,
                  and what drives the run. */}
              <div className="chrome-group">
                <button disabled={!editable} onClick={newGraph}>New</button>
                <button disabled={!editable} onClick={openFile}>Open</button>
                <button disabled={!editable} onClick={() => save(false)}>Save</button>
                <button disabled={!editable} onClick={() => save(true)}>Save as</button>
              </div>
              <div className="chrome-group">
                <button disabled={!controls.start} onClick={() => actOnRun('start_run')}>
                  Start
                </button>
                <button disabled={!controls.stop} onClick={() => actOnRun('stop_run')}>
                  Stop
                </button>
                <button disabled={!controls.reset} onClick={() => actOnRun('reset_run')}>
                  Reset
                </button>
              </div>
            </header>
            {/* The banner names the loss without covering the canvas: the
                view beneath stays where the user left it, looking live. */}
            <Banner text={banner} />
            <div className="workspace">
              <Palette
                types={listing?.types}
                editable={editable}
                onCreate={createFromPalette}
              />
              <main className="canvas" ref={canvasRef}>
                {/* The plugin UI contract rides the canvas subtree alone: its
                    readers are the node renderings, nothing in the chrome. */}
                <PluginUiContext.Provider
                  value={{ h: createElement, bodies, valueBodies, failBody, failValue }}
                >
                  <ReactFlow
                    nodes={nodes}
                    edges={graph === null ? [] : toEdges(graph, pulsing, listing, facts)}
                    nodeTypes={nodeTypes}
                    edgeTypes={edgeTypes}
                    onNodesChange={onNodesChange}
                    onConnect={onConnect}
                    onConnectEnd={onConnectEnd}
                    onNodeContextMenu={onNodeContextMenu}
                    onPaneContextMenu={onPaneContextMenu}
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
                    // React Flow's is retired, as is edge focus: a wire is
                    // not operable, and takes no seat in the tab order. A
                    // wire is a drag from either end — a click never starts
                    // or lands one — and the drag threshold keeps a port
                    // click from reading as a drag-off.
                    multiSelectionKeyCode="Shift"
                    selectionKeyCode={null}
                    deleteKeyCode={null}
                    edgesFocusable={false}
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
                    Drag a node type from the palette onto the canvas, or
                    press Enter on a palette row.
                  </div>
                )}
                <Toasts toasts={toasts} onDismiss={dismissToast} />
                {menu !== null && (
                  <ContextMenu
                    x={menu.x}
                    y={menu.y}
                    entries={entries}
                    onClose={() => setMenu(null)}
                  />
                )}
              </main>
              <Sidebar
                node={selected}
                type={selectedType}
                wiredInputs={selectedWiredInputs}
                baseScalars={listing?.baseScalars ?? {}}
              />
              {selectedNodes.length > 1 && (
                <SelectionSidebar
                  nodes={selectedNodes}
                  types={selectionTypes}
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
        </WireContext.Provider>
      </LockContext.Provider>
    </EditContext.Provider>
  )
}

