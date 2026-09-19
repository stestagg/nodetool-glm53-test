// The editor's context menu: the selection's gestures where the pointer
// already is. The shell opens it on a right-click — on a node in the
// selection, or on an unselected node it selects alone first — and the
// menu acts on the selection as it stands: opening never disturbs it.
// It closes on a pick, on Escape, and on a press outside it, and its
// items are buttons in the tab order, so the keyboard that started here
// can finish here too; the chords (keyboard.js's groupKeys) issue the
// same operations without it.

import { useEffect, useRef } from 'react'

export function ContextMenu({ x, y, entries, onClose }) {
  const ref = useRef(null)
  useEffect(() => {
    ref.current?.querySelector('button')?.focus()
  }, [])
  useEffect(() => {
    const onKey = (event) => {
      if (event.key === 'Escape') onClose()
    }
    const onDown = (event) => {
      if (!ref.current?.contains(event.target)) onClose()
    }
    document.addEventListener('keydown', onKey, true)
    document.addEventListener('mousedown', onDown, true)
    return () => {
      document.removeEventListener('keydown', onKey, true)
      document.removeEventListener('mousedown', onDown, true)
    }
  }, [onClose])
  return (
    <div className="menu" role="menu" style={{ left: x, top: y }} ref={ref}>
      {entries.map((entry) => (
        <button
          key={entry.key}
          role="menuitem"
          onClick={() => {
            entry.onPick()
            onClose()
          }}
        >
          {entry.label}
        </button>
      ))}
    </div>
  )
}
