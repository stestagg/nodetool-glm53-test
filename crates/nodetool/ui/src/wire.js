// The editor's wire: the one connection to the server, holding what the
// socket delivers as the browser's state — the browser holds no
// authoritative graph state, it renders what the server pushed and every
// edit lands server-side. The greeting opens the exchange: the listing
// and the definition requested, the connection named. What each piece of
// the delivery means — the definition, the connection states, the run
// display, the coalescer, the run control's act — lives in the comment
// above the code that carries it.

import { useCallback, useEffect, useRef, useState } from 'react'
import { connect } from './protocol.js'
import { RunCoalescer } from './coalesce.js'
import { canvasValues, groupFacts } from './groups.js'
import { emittedTargets, runStatusText } from './view.js'

export function useEditorWire(showToast) {
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
  const [status, setStatus] = useState({ text: 'connecting…', error: false })
  const protocol = useRef(null)
  // The held definition's edges, as the event path reads them: the
  // emission handler lives across renders, and the wires it pulses are
  // the ones the last resync carried.
  const graphRef = useRef(null)
  // The held definition's groups, as the event path and the render read
  // them: which inner emissions cross a boundary, which stay inside,
  // which statuses aggregate — composed once per definition arrival.
  const factsRef = useRef(null)
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

  useEffect(() => () => coalescer.current.dispose(), [])

  // A definition arrival — the connect-time resync or a push — replaces
  // what is drawn, and the run state it carries narrates itself the way a
  // run push does. An idle state with no outcome — what a compile failure
  // leaves — reads as '', leaving the status line, the refused start's
  // error report, as it is. The run display is not part of it: the
  // statuses and values arrive as their own push, on the ordered stream
  // the live changes ride.
  const onDefinition = useCallback((graph, file, run, problems) => {
    graphRef.current = graph
    factsRef.current = groupFacts(graph)
    setGraph(graph)
    setFile(file)
    setRun(run)
    setProblems(problems ?? [])
    setActing(false)
    const text = runStatusText(run)
    if (text) setStatus({ text, error: run.outcome === 'failed' })
  }, [])

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

  useEffect(
    () =>
      connect({
        onOpen: (request) => {
          protocol.current = request
          request('list_node_types').then(setListing, showToast)
          request('get_definition').then(
            ({ graph, file, run, problems }) => onDefinition(graph, file, run, problems),
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
        onDefinition,
        onFile: ({ path, dirty }) => setFile({ path, dirty }),
        // The run display arrives as its own push — the connect-time
        // resync's snapshot and nothing else; the live changes that keep
        // it true ride the same ordered stream behind it. A group's
        // boundary values are re-keyed to the instance port that shows
        // them, and the inside's stay inside.
        onRunDisplay: ({ statuses, values }) => {
          setStatuses(statuses ?? {})
          coalescer.current.replace(canvasValues(values, factsRef.current))
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
          // emitting port, the wires the value travels. An inner node's
          // emission animates the boundary when it is the one an exposed
          // output binds, and stays inside otherwise — collapsed means
          // collapsed.
          if (message.event !== 'emitted') return
          const facts = factsRef.current
          const boundary = facts?.emissions.get(`${message.node}/${message.port}`)
          if (boundary !== undefined) {
            coalescer.current.emitted(
              boundary.instance,
              boundary.port,
              message.value,
              emittedTargets(graphRef.current?.edges ?? [], boundary.instance, boundary.port),
            )
          } else if (!facts?.inner.has(message.node)) {
            coalescer.current.emitted(
              message.node,
              message.port,
              message.value,
              emittedTargets(graphRef.current?.edges ?? [], message.node, message.port),
            )
          }
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
    [onDefinition, showToast],
  )

  return {
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
    coalescer,
    actOnRun,
  }
}
