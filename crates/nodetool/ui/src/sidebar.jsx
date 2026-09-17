// The editing sidebar: the right-hand dock the shell keeps the edge for.
// It appears with a selected node and closes when the selection clears,
// so an unselected canvas keeps its full width; selecting another node
// swaps the contents. The node's global attributes sit on top — the
// label, editable to any name, empty meaning the type's default; the
// type reference and uuid read-only beneath it for orientation — and the
// parameter fields below, one per scalar-possible input, a wire's
// landing replacing the field with the declared types. Everything
// renders from the server-held definition — the same view the canvas
// draws — and every commit is an operation answered by a push that
// re-renders both views. A node whose type reference is missing from the
// listing shows label editing only: no fields are invented for a type
// the editor cannot see.

import { commitParameter, Field, scalarPossible, useEdit } from './fields.jsx'

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
