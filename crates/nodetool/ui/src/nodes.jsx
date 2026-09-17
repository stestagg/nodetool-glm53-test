// The node renderings. The default node class draws a node from its type
// definition alone — a title bar carrying the node's current label,
// named input ports down the left, named output ports down the right,
// each port's declared type available as a tooltip rather than printed
// on the node, because space is at a premium. A scalar-possible input
// also carries a small editable field on the node itself — the second
// view of the input's stored value, the sidebar the first; a wire
// reaching the input replaces the field with the declared types, since a
// connected input holds no literal to type into. It works when a plugin
// provides nothing but the definition. A node whose type reference is
// missing from the listing renders as a placeholder instead — ports
// unknown, so no fields, but the label is the one attribute its sidebar
// can still edit.

import { Handle, Position } from '@xyflow/react'
import { commitParameter, Field, useEdit } from './fields.jsx'

function InputPort({ port, node, wired, scalar, title }) {
  const edit = useEdit()
  return (
    <div
      className={`port in${wired ? ' connected' : ''}`}
      title={port.type_refs.join(', ')}
    >
      <Handle type="target" position={Position.Left} id={port.name} />
      <span className="port-name">{port.name}</span>
      {scalar &&
        (wired ? (
          <span className="port-connected" title="connected">
            {port.type_refs.join(', ')}
          </span>
        ) : (
          <Field
            className="port-field"
            value={node.parameters?.[port.name]}
            aria-label={`${title} ${port.name}`}
            onCommit={(text) => commitParameter(edit, node.uuid, port.name, text)}
          />
        ))}
    </div>
  )
}

function OutputPort({ port }) {
  return (
    <div className="port out" title={port.type_refs.join(', ')}>
      <Handle type="source" position={Position.Right} id={port.name} />
      <span className="port-name">{port.name}</span>
    </div>
  )
}

export function TypeNode({ data, selected }) {
  const { node, type, wiredInputs, scalarInputs } = data
  const title = node.label ?? type.label
  return (
    <div className={`node${selected ? ' selected' : ''}`}>
      <div className="node-title">{title}</div>
      <div className="node-ports">
        <div className="node-inputs">
          {type.inputs.map((port) => (
            <InputPort
              key={port.name}
              port={port}
              node={node}
              wired={wiredInputs.includes(port.name)}
              scalar={scalarInputs.includes(port.name)}
              title={title}
            />
          ))}
        </div>
        <div className="node-outputs">
          {type.outputs.map((port) => (
            <OutputPort key={port.name} port={port} />
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
