// The node renderings. The default node class draws a node from its type
// definition alone — a title bar carrying the node's current label, named
// input ports down the left, named output ports down the right, each
// port's declared type available as a tooltip rather than printed on the
// node, because space is at a premium. It works when a plugin provides
// nothing but the definition. A node whose type reference is missing from
// the listing renders as an inert placeholder instead — never silently
// dropped.

import { Handle, Position } from '@xyflow/react'

function Port({ port, kind }) {
  const incoming = kind === 'in'
  return (
    <div className={`port ${kind}`} title={port.type_refs.join(', ')}>
      <Handle
        type={incoming ? 'target' : 'source'}
        position={incoming ? Position.Left : Position.Right}
        id={port.name}
        isConnectable={false}
      />
      <span className="port-name">{port.name}</span>
    </div>
  )
}

export function TypeNode({ data, selected }) {
  const { node, type } = data
  return (
    <div className={`node${selected ? ' selected' : ''}`}>
      <div className="node-title">{node.label ?? type.label}</div>
      <div className="node-ports">
        <div className="node-inputs">
          {type.inputs.map((port) => (
            <Port key={port.name} port={port} kind="in" />
          ))}
        </div>
        <div className="node-outputs">
          {type.outputs.map((port) => (
            <Port key={port.name} port={port} kind="out" />
          ))}
        </div>
      </div>
    </div>
  )
}

export function PlaceholderNode({ data }) {
  return (
    <div className="node placeholder">
      <div className="node-title">{data.label}</div>
      <div className="node-unknown">unknown type — ports unknown</div>
    </div>
  )
}
