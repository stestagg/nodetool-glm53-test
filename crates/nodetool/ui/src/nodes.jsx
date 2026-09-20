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
// A type's declared choices — its per-instance settings, each one of a
// fixed set of options — render as selects in the middle of the node,
// between the title bar and the ports, where a Blender-style node keeps
// its controls. The chosen option is on the canvas, so a graph of them
// reads without opening each node; the sidebar shows the same selects, the
// two views of one stored value every parameter keeps.
//
// A type that declares custom node UI renders its bundle's component once
// the bundle loads, owning the body between the title bar and the ports:
// the shell — the title bar with its label, icon, and status, and the
// ports with their fields and readouts — stays editor-rendered, so every
// gesture lands on structure the editor drew and behaves exactly as on a
// default node. The component receives the contract (pluginui.jsx): the
// node's definition slice, the live display state, and the two edit
// helpers. Until the bundle arrives, and on every failure after it —
// reported, naming the type — the node renders by the default class.
//
// While a run touches a node, the title bar carries its derived status —
// the bridge's one transition table, rendered by name, never recomputed
// here — and an output port with a latest value shows it beside the port
// name, replaced by each new emission. A value whose type declares value
// UI displays through the plugin's component once that bundle loads;
// until then, and on every failure, the readout stays contentless rather
// than showing content of the editor's inventing — scalar values keep
// their plain text, as they always have. Both persist after the run
// ends, until the next start resets them; the statuses are states on the
// node, not decoration.
//
// Both renderings wear the marks the compile's problems name: one mark
// per node, every message that names the node readable at it — a
// warning's or an error's, and a failed run's explanation the same way.
// The mark is the dot at the corner, the glanceable flag; the messages
// print on the default class's node itself, and ride the custom contract
// for the plugin's rendering to place. Marks advise: they disable
// nothing.
//
// The type's declared icon rides the title bar beside the label, and the
// ports carry the type channel: a port declaring exactly one type renders
// its dot in that type's declared colour and shape, everything else — a
// union declaration, a reference the listing does not know — in the
// neutral pair. The declared types remain readable in the port's
// tooltip.

import { useContext } from 'react'
import { Handle, Position } from '@xyflow/react'
import { commitParameter, Field, Select, useEdit, useLocked, WireContext } from './fields.jsx'
import { portKey } from './keyboard.js'
import { PluginUiContext, UiBoundary } from './pluginui.jsx'
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

// The port's keyboard concern in one place, one context read per port:
// the keys turned into the operation each pointer gesture sends — Delete
// on a connected input unhooks, Enter or Space starts or lands a
// keyboard wire — and the wire-from test marking the port a keyboard
// wire runs from. The decision is keyboard.js's; the lock holds here the
// way it holds every port's pointer gesture.
function usePortKeys() {
  const edit = useEdit()
  const locked = useLocked()
  const { wire, setWire } = useContext(WireContext)
  return {
    act: (event, port) => {
      const act = portKey(event, port, wire, !locked)
      if (act === null) return
      event.preventDefault()
      event.stopPropagation()
      if (act.kind === 'unhook') edit('unhook', act.fields)
      else if (act.kind === 'start') setWire({ node: port.node, port: port.port, type: port.type })
      else {
        setWire(null)
        if (act.fields) edit('wire', act.fields)
      }
    },
    wireFrom: (nodeUuid, portName, type) =>
      wire?.node === nodeUuid && wire?.port === portName && wire?.type === type,
  }
}

function InputPort({ port, node, wired, scalar, appearance, title }) {
  const edit = useEdit()
  const { act: onPortKey, wireFrom } = usePortKeys()
  return (
    <div
      className={`port in${wired ? ' connected' : ''}`}
      title={port.type_refs.join(', ')}
    >
      <Handle
        type="target"
        position={Position.Left}
        id={port.name}
        className={`${appearance.shape}${wireFrom(node.uuid, port.name, 'target') ? ' wire-from' : ''}`}
        style={{ background: appearance.color, borderColor: appearance.color }}
        tabIndex={0}
        aria-label={`${title} ${port.name} input`}
        onKeyDown={(event) =>
          onPortKey(event, { node: node.uuid, port: port.name, type: 'target', wired })
        }
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

// The readout at one output port: a scalar value as its plain text, and a
// value whose single declared type declares value UI through the plugin's
// component — loaded by now, since a displayed value is what asks for the
// bundle; before it arrives, and after any failure, the readout stays
// contentless rather than showing a rendering of the editor's inventing.
function PortValue({ port, dataTypes, value }) {
  const plugin = useContext(PluginUiContext)
  if (value === undefined) return null
  const ref = port.type_refs.length === 1 ? port.type_refs[0] : null
  if (!ref || !dataTypes?.[ref]?.ui) {
    return (
      <span className="port-value" title={value}>
        {value}
      </span>
    )
  }
  const Body = plugin?.valueBodies?.[ref]
  if (!Body) return null
  return (
    <UiBoundary onFail={(error) => plugin.failValue(ref, error)}>
      <span className="port-value" title={value}>
        <Body h={plugin.h} value={value} />
      </span>
    </UiBoundary>
  )
}

function OutputPort({ port, appearance, value, dataTypes, nodeUuid, title }) {
  const { act: onPortKey, wireFrom } = usePortKeys()
  return (
    <div className="port out" title={port.type_refs.join(', ')}>
      <Handle
        type="source"
        position={Position.Right}
        id={port.name}
        className={`${appearance.shape}${wireFrom(nodeUuid, port.name, 'source') ? ' wire-from' : ''}`}
        style={{ background: appearance.color, borderColor: appearance.color }}
        tabIndex={0}
        aria-label={`${title} ${port.name} output`}
        onKeyDown={(event) =>
          onPortKey(event, { node: nodeUuid, port: port.name, type: 'source', wired: false })
        }
      />
      <span className="port-name">{port.name}</span>
      <PortValue port={port} dataTypes={dataTypes} value={value} />
    </div>
  )
}

// The declared choices, as the node shows them: one select per setting,
// named beside it, committing through the same parameter edit every field
// commits through.
function NodeChoices({ choices, node, title }) {
  const edit = useEdit()
  if (choices.length === 0) return null
  return (
    <div className="node-choices">
      {choices.map((choice) => (
        <div className="node-choice" key={choice.name}>
          <span className="choice-name">{choice.name}</span>
          <Select
            className="choice-select"
            value={node.parameters?.[choice.name]}
            options={choice.options}
            aria-label={`${title} ${choice.name}`}
            onCommit={(text) => commitParameter(edit, node.uuid, choice.name, text)}
          />
        </div>
      ))}
    </div>
  )
}

export function TypeNode({ data, selected }) {
  const { node, type, wiredInputs, scalarInputs, marks, status, portValues, dataTypes } = data
  const plugin = useContext(PluginUiContext)
  const edit = useEdit()
  const locked = useLocked()
  const title = node.label ?? type.label
  const Body = plugin?.bodies?.[type.type_ref]
  // The plugin contract: the node's definition slice and the live display
  // state beside the two edits the component may issue — the same
  // operations the sidebar and the inline fields commit, no second path.
  const contract = {
    h: plugin?.h,
    label: title,
    parameters: node.parameters ?? {},
    connectedInputs: wiredInputs,
    status,
    marks: marks ?? [],
    portValues: portValues ?? {},
    locked,
    setLabel: (text) => edit('set_label', { uuid: node.uuid, label: text }),
    setParameter: (input, text) => commitParameter(edit, node.uuid, input, text),
  }
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
      <NodeChoices choices={type.choices ?? []} node={node} title={title} />
      {Body && (
        <UiBoundary onFail={(error) => plugin.failBody(type.type_ref, error)}>
          <div className="node-body">
            <Body {...contract} />
          </div>
        </UiBoundary>
      )}
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
              dataTypes={dataTypes}
              nodeUuid={node.uuid}
              title={title}
            />
          ))}
        </div>
      </div>
      {!Body && marks?.length > 0 && (
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
