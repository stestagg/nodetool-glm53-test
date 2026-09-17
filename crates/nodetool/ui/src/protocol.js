// The websocket protocol client. One connection carries every exchange:
// each request carries an id its reply echoes, a reply is either the answer
// or an error naming the problem, and pushes ride the same envelope with no
// id. The browser holds no graph state of its own — a definition push is
// rendered as it arrives, without applying or merging anything.

let nextId = 1

// The drag-and-drop marker the palette stamps and the canvas reads.
export const NODE_TYPE = 'application/x-nodetool-node-type'

// Open the editor's connection. `onOpen(request)` receives the request
// function once the socket is live; `onGreeting` fires when the server's
// greeting push arrives; the others receive pushes and connection events.
// Returns a function that closes the connection.
export function connect({ onOpen, onGreeting, onDefinition, onError, onClosed }) {
  const socket = new WebSocket(
    `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`,
  )
  const pending = new Map()

  const settle = (id, answer) => {
    const waiting = pending.get(id)
    if (waiting === undefined) {
      onError(`a reply arrived for unknown request id ${JSON.stringify(id)}`)
      return
    }
    pending.delete(id)
    answer(waiting)
  }

  socket.onmessage = (event) => {
    let message
    try {
      message = JSON.parse(event.data)
    } catch {
      console.error('the server sent a message that is not JSON')
      return
    }
    switch (message.type) {
      case 'greeting':
        onGreeting()
        break
      case 'definition':
        if (message.id === undefined) onDefinition(message.graph)
        else settle(message.id, ({ resolve }) => resolve(message.graph))
        break
      case 'node_types':
        // The listing and its base-scalar fact ride one reply.
        settle(message.id, ({ resolve }) =>
          resolve({ types: message.node_types, baseScalars: message.base_scalars }))
        break
      case 'node_created':
      case 'node_moved':
      case 'label_set':
      case 'parameter_set':
      case 'wired':
      case 'unhooked':
      case 'node_deleted':
        settle(message.id, ({ resolve }) => resolve(message))
        break
      case 'error':
        if (message.id === undefined) onError(message.error)
        else settle(message.id, ({ reject }) => reject(message.error))
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
    // Requests still in flight will never see their replies; settling them
    // here lets the empty states draw instead of waiting forever.
    for (const waiting of pending.values()) waiting.reject('connection lost')
    pending.clear()
    onClosed()
  }

  return () => socket.close()
}
