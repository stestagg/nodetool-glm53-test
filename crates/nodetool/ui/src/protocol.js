// The websocket protocol client. One connection carries every exchange:
// each request carries an id its reply echoes, a reply is either the answer
// or an error naming the problem, and pushes ride the same envelope with no
// id. The browser holds no graph state of its own — a definition push is
// rendered as it arrives, without applying or merging anything. A connection
// that dies is not the end: the client keeps trying, and the tab rejoins
// through the same path as a first connect. Only a greeting whose versions
// mismatch the page's stops the trying — against a server the page no
// longer understands, reloading the page is the advice.

let nextId = 1

// The versions this page speaks, mirroring the server it was built
// against — nodetool::server::PROTOCOL_VERSION and
// nodetool::graph::SCHEMA_VERSION. A mismatch is told, never misbehaved
// against.
export const PROTOCOL_VERSION = 1
export const SCHEMA_VERSION = 1

// How long after a loss the trying resumes.
const RETRY_MS = 1000

// The drag-and-drop marker the palette stamps and the canvas reads.
export const NODE_TYPE = 'application/x-nodetool-node-type'

// Open the editor's connection, retrying it until it holds. `onOpen(request)`
// receives the request function once the socket is live; `onGreeting` fires
// when the server's greeting push arrives; the others receive pushes and
// connection events. `onVersionMismatch` carries the mismatch the greeting
// named, and is the one ending the retrying. `onClosed` fires on every
// loss that follows — never on a mismatch or a deliberate close. Returns a
// function that closes the connection for good.
export function connect({
  onOpen,
  onGreeting,
  onVersionMismatch,
  onDefinition,
  onFile,
  onRun,
  onError,
  onClosed,
}) {
  let socket = null
  let stopped = false

  const open = () => {
    if (stopped) return
    const pending = new Map()
    socket = new WebSocket(
      `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`,
    )
    socket.onmessage = (event) => {
      let message
      try {
        message = JSON.parse(event.data)
      } catch {
        onError('the server sent a message that is not JSON')
        return
      }
      switch (message.type) {
        case 'greeting':
          if (
            message.protocol_version !== PROTOCOL_VERSION ||
            message.schema_version !== SCHEMA_VERSION
          ) {
            stopped = true
            socket.close()
            onVersionMismatch(
              `the server speaks protocol ${message.protocol_version} and schema ${message.schema_version}, this page speaks protocol ${PROTOCOL_VERSION} and schema ${SCHEMA_VERSION}`,
            )
            return
          }
          onGreeting()
          break
        case 'definition':
          if (message.id === undefined)
            onDefinition(message.graph, message.file, message.run, message.problems)
          else
            settle(pending, onError, message.id, ({ resolve }) =>
              resolve({
                graph: message.graph,
                file: message.file,
                run: message.run,
                problems: message.problems,
              }))
          break
        case 'file':
          onFile(message)
          break
        case 'run':
          onRun(message)
          break
        case 'node_types':
          // The listing and its base-scalar fact ride one reply.
          settle(pending, onError, message.id, ({ resolve }) =>
            resolve({ types: message.node_types, baseScalars: message.base_scalars }))
          break
        case 'node_created':
        case 'node_moved':
        case 'label_set':
        case 'parameter_set':
        case 'wired':
        case 'unhooked':
        case 'node_deleted':
        case 'file_opened':
        case 'file_saved':
        case 'graph_created':
        case 'run_started':
        case 'run_stopped':
          settle(pending, onError, message.id, ({ resolve }) => resolve(message))
          break
        case 'error':
          if (message.id === undefined) onError(message.error)
          else settle(pending, onError, message.id, ({ reject }) => reject(message.error))
          break
        default:
          onError(`the server sent an unknown message type ${JSON.stringify(message.type)}`)
      }
    }
    socket.onopen = () => {
      onOpen((type, fields = {}) => {
        const id = nextId++
        return new Promise((resolve, reject) => {
          pending.set(id, { resolve, reject })
          socket.send(JSON.stringify({ id, type, ...fields }))
        })
      })
    }
    socket.onclose = () => {
      // Requests still in flight will never see their replies; settling
      // them here lets the empty states draw instead of waiting forever.
      for (const waiting of pending.values()) waiting.reject('connection lost')
      pending.clear()
      if (stopped) return
      onClosed()
      setTimeout(open, RETRY_MS)
    }
  }

  open()

  return () => {
    stopped = true
    socket?.close()
  }
}

function settle(pending, onError, id, answer) {
  const waiting = pending.get(id)
  if (waiting === undefined) {
    onError(`a reply arrived for unknown request id ${JSON.stringify(id)}`)
    return
  }
  pending.delete(id)
  answer(waiting)
}
