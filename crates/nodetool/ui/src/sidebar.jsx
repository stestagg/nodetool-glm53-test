// The editing sidebar: the right-hand dock the shell keeps the edge for.
// It appears with a selection and closes when the selection clears, so an
// unselected canvas keeps its full width. One selected node is the node's
// own view: the label, editable to any name, empty meaning the type's
// default; the type reference and uuid read-only beneath it for
// orientation; and the parameter fields below, one per scalar-possible
// input, a wire's landing replacing the field with the declared types.
// Several selected nodes are the selection's view: a header naming the
// selection by its count, then only the fields the nodes hold in common,
// each commit applying to every node — no label, no uuid, no type
// reference, a label naming one node and the facts being per-node.
// Everything renders from the server-held definition — the same view the
// canvas draws — and every commit is an operation answered by a push that
// re-renders both views. A node whose type reference is missing from the
// listing shows label editing only: no fields are invented for a type
// the editor cannot see.

import { SelectionField, commitParameter, Field, scalarPossible, useEdit } from './fields.jsx'
import { commonFields, commitCommon } from './selection.js'

export function Sidebar({ node, type, wiredInputs, baseScalars }) {
  const edit = useEdit()
  if (node === undefined) return null
  const scalarPorts = (type?.inputs ?? []).filter((port) =>
    scalarPossible(port, baseScalars),
  )
  return (
    <aside className="sidebar">
      <section className="sidebar-section">
        <h1 className="sidebar-heading">Node</h1>
        <label className="sidebar-row">
          <span className="sidebar-name">Label</span>
          <Field
            className="sidebar-value"
            value={node.label ?? ''}
            placeholder={type?.label ?? node.type_ref}
            onCommit={(text) => edit('set_label', { uuid: node.uuid, label: text })}
          />
        </label>
        <div className="sidebar-fact">
          <span className="sidebar-name">Type</span>
          <code title={node.type_ref}>{node.type_ref}</code>
        </div>
        <div className="sidebar-fact">
          <span className="sidebar-name">UUID</span>
          <code title={node.uuid}>{node.uuid}</code>
        </div>
      </section>
      {scalarPorts.length > 0 && (
        <section className="sidebar-section">
          <h1 className="sidebar-heading">Parameters</h1>
          {scalarPorts.map((port) => (
            <div className="sidebar-row" key={port.name}>
              <span className="sidebar-name">{port.name}</span>
              {wiredInputs.includes(port.name) ? (
                <span className="sidebar-connected">
                  connected — {port.type_refs.join(', ')}
                </span>
              ) : (
                <Field
                  className="sidebar-value"
                  value={node.parameters?.[port.name]}
                  aria-label={`${node.label ?? type.label} ${port.name}`}
                  onCommit={(text) =>
                    commitParameter(edit, node.uuid, port.name, text)
                  }
                />
              )}
            </div>
          ))}
        </section>
      )}
    </aside>
  )
}

// The multi-selection's view of the sidebar. The header names the
// selection by its count; the fields are the selection's common ones —
// each shown at its shared value, or marked mixed where the nodes
// differ — and a commit lands on every node through the per-node
// operation each one is. Where nothing is common the sidebar says so
// plainly rather than sitting silently empty.
export function SelectionSidebar({ nodes, types, baseScalars, edges }) {
  const edit = useEdit()
  const fields = commonFields(nodes, types, baseScalars, edges)
  return (
    <aside className="sidebar">
      <section className="sidebar-section">
        <h1 className="sidebar-heading">{nodes.length} nodes selected</h1>
        {fields.length === 0 ? (
          <p className="sidebar-empty">no shared parameters</p>
        ) : (
          fields.map((field) => (
            <div className="sidebar-row" key={field.name}>
              <span className="sidebar-name">{field.name}</span>
              <SelectionField
                text={field.text}
                mixed={field.mixed}
                aria-label={field.name}
                onCommit={(value) => commitCommon(edit, nodes, field.name, value)}
              />
            </div>
          ))
        )}
      </section>
    </aside>
  )
}
