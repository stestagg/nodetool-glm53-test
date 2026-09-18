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
// can still edit, and the unknown-type problem it carries explains why.
//
// While a run touches a node, the title bar carries its derived status —
// the bridge's one transition table, rendered by name, never recomputed
// here — and an output port with a latest value shows it as small text
// beside the port name, replaced by each new emission. Both persist
// after the run ends, until the next start resets them; the statuses are
// states on the node, not decoration.
//
// Both renderings wear the marks the compile's problems name: one mark
// per node, every message that names the node readable at it — a
// warning's or an error's, and a failed run's explanation the same way.
// The mark is the dot at the corner, the glanceable flag; the messages
// print on the node itself, where they can be read, not only hovered.
// Marks advise: they disable nothing.
//
// The type's declared icon rides the title bar beside the label, and the
// ports carry the type channel: a port declaring exactly one type renders
// its dot in that type's declared colour and shape, everything else — a
// union declaration, a reference the listing does not know — in the
// neutral pair. The declared types remain readable in the port's
// tooltip.

import { Handle, Position } from '@xyflow/react'
import { commitParameter, Field, useEdit } from './fields.jsx'
import { portAppearance } from './types.js'

// The status state in the title bar, right of the label: one word, the
// bridge's own vocabulary.
export function NodeStatus({ status }) {
  if (status === undefined) return null
  return <span className={`node-status ${status}`}>{status}</span>
}

// The type's declared inline SVG, wherever the type shows: the palette
// row and the node title bar. Rendered as declared — a plugin is
// build-time linked code, and a malformed or empty icon is its
// declaration to fix, not an input to sanitise.
export function TypeIcon({ icon }) {
  if (!icon) return null
  return <span className="type-icon" dangerouslySetInnerHTML={{ __html: icon }} />
}

function InputPort({ port, node, wired, scalar, appearance, title }) {
  const edit = useEdit()
  return (
    <div
      className={`port in${wired ? ' connected' : ''}`}
      title={port.type_refs.join(', ')}
    >
      <Handle
        type="target"
        position={Position.Left}
        id={port.name}
        className={appearance.shape}
        style={{ background: appearance.color, borderColor: appearance.color }}
      />
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

function OutputPort({ port, appearance, value }) {
  return (
    <div className="port out" title={port.type_refs.join(', ')}>
      <Handle
        type="source"
        position={Position.Right}
        id={port.name}
        className={appearance.shape}
        style={{ background: appearance.color, borderColor: appearance.color }}
      />
      <span className="port-name">{port.name}</span>
      {value !== undefined && (
        <span className="port-value" title={value}>
          {value}
        </span>
      )}
    </div>
  )
}

export function TypeNode({ data, selected }) {
  const { node, type, wiredInputs, scalarInputs, marks, status, portValues, dataTypes } = data
  const title = node.label ?? type.label
  return (
    <div className={`node${selected ? ' selected' : ''}`}>
      {marks?.length > 0 && <div className="node-mark" />}
      <div className="node-title">
        <span className="node-heading">
          <TypeIcon icon={type.icon} />
          {title}
        </span>
        <NodeStatus status={status} />
      </div>
      <div className="node-ports">
        <div className="node-inputs">
          {type.inputs.map((port) => (
            <InputPort
              key={port.name}
              port={port}
              node={node}
              wired={wiredInputs.includes(port.name)}
              scalar={scalarInputs.includes(port.name)}
              appearance={portAppearance(port, dataTypes)}
              title={title}
            />
          ))}
        </div>
        <div className="node-outputs">
          {type.outputs.map((port) => (
            <OutputPort
              key={port.name}
              port={port}
              appearance={portAppearance(port, dataTypes)}
              value={portValues?.[port.name]}
            />
          ))}
        </div>
      </div>
      {marks?.length > 0 && (
        <div className="node-problem" title={marks.join('\n')}>
          {marks.join('\n')}
        </div>
      )}
    </div>
  )
}

export function PlaceholderNode({ data, selected }) {
  const marks = data.marks ?? []
  return (
    <div className={`node placeholder${selected ? ' selected' : ''}`}>
      <div className="node-title">
        {data.label}
        <NodeStatus status={data.status} />
      </div>
      <div className="node-unknown">unknown type — ports unknown</div>
      {marks.length > 0 && (
        <div className="node-problem" title={marks.join('\n')}>
          {marks.join('\n')}
        </div>
      )}
    </div>
  )
}
