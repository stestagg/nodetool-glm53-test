// The palette: one section per plugin headed by the plugin's name, a
// headed sub-section per declared sub-group within it, the ungrouped
// types directly under the header — plugins, sub-groups, and types
// alphabetical, every tab and reload agreeing. A row is a drag source,
// and a keyboard path beside it: the row takes focus and Enter or Space
// creates the node through the same create operation a drop sends, at
// the position the shell answers with. While the lock holds, the rows
// go quiet exactly with their pointer twin — the drag — the same
// disabled reading the file controls get.

import { NODE_TYPE } from './protocol.js'
import { TypeIcon } from './nodes.jsx'

// The palette's sections, from the listing's plugin and sub-group facts:
// one section per plugin, a headed sub-section per declared sub-group
// within it, the ungrouped types directly under the plugin header.
// Plugins, sub-groups, and types order alphabetically, a type's label tie
// broken by its type reference — a deterministic order every tab and
// reload agrees on.
export function paletteGroups(types) {
  const plugins = new Map()
  for (const type of types) {
    const subGroup = type.sub_group ?? null
    const sections = plugins.get(type.plugin) ?? new Map()
    sections.set(subGroup, [...(sections.get(subGroup) ?? []), type])
    plugins.set(type.plugin, sections)
  }
  return [...plugins.keys()].sort(byName).map((plugin) => {
    const sections = plugins.get(plugin)
    return {
      plugin,
      sections: [...sections.keys()]
        .sort((a, b) => (a === null ? -1 : b === null ? 1 : byName(a, b)))
        .map((subGroup) => ({
          subGroup,
          types: sections
            .get(subGroup)
            .sort((a, b) => byName(a.label, b.label) || byName(a.type_ref, b.type_ref)),
        })),
    }
  })
}

const byName = (a, b) => (a < b ? -1 : a > b ? 1 : 0)

// Enter or Space — the two activation keys, whichever the hand came from.
function activated(event) {
  return event.key === 'Enter' || event.key === ' '
}

export function Palette({ types, editable, onCreate }) {
  return (
    <aside className="palette">
      <h1 className="palette-title">Nodes</h1>
      {types === undefined ? null : types.length === 0 ? (
        <p className="palette-empty">
          No node types are linked into this binary. Link a plugin crate
          to see its types here.
        </p>
      ) : (
        paletteGroups(types).map((group) => (
          <section className="palette-plugin" key={group.plugin}>
            <h2 className="palette-heading">{group.plugin}</h2>
            {group.sections.map((section) => (
              <div className="palette-group" key={section.subGroup ?? ''}>
                {section.subGroup !== null && (
                  <h3 className="palette-subgroup">{section.subGroup}</h3>
                )}
                <ul className="palette-list">
                  {section.types.map((type) => (
                    <li
                      key={type.type_ref}
                      className="palette-item"
                      draggable={editable}
                      tabIndex={editable ? 0 : -1}
                      aria-disabled={editable ? undefined : true}
                      onKeyDown={(event) => {
                        if (editable && activated(event)) {
                          event.preventDefault()
                          onCreate(type.type_ref)
                        }
                      }}
                      onDragStart={(event) => {
                        event.dataTransfer.setData(NODE_TYPE, type.type_ref)
                        event.dataTransfer.effectAllowed = 'move'
                      }}
                    >
                      <TypeIcon icon={type.icon} />
                      <span className="palette-label">{type.label}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </section>
        ))
      )}
    </aside>
  )
}
