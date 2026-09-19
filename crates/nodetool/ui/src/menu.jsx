// The editor's context menu: the selection's gestures where the pointer
// already is. The shell opens it on a right-click — on a node in the
// selection, or on an unselected node it selects alone first — and the
// menu acts on the selection it opened for: opening never disturbs it.
// It closes on a pick, on Escape, and on a press outside it, and its
// items are buttons in the tab order, so the keyboard that started here
// can finish here too; the chords (keyboard.js's groupKeys) issue the
// same operations without it. Seated where the pointer is, clamped to
// stay inside the canvas — a right-click near an edge opens a menu the
// pointer can still reach.

import { useEffect, useLayoutEffect, useRef } from 'react'

export function ContextMenu({ x, y, entries, onClose }) {
  const ref = useRef(null)
  // The seat is the pointer's, but the menu must be reachable: past the
  // canvas's edge it flips back inside, the way a native menu does —
  // clamped once, before paint, against the container it renders in,
  // measured at its real size.
  useLayoutEffect(() => {
    const node = ref.current
    const canvas = node?.parentElement
    if (node === null || canvas === null) return
    node.style.left = `${Math.max(0, Math.min(x, canvas.clientWidth - node.offsetWidth))}px`
    node.style.top = `${Math.max(0, Math.min(y, canvas.clientHeight - node.offsetHeight))}px`
  }, [x, y])
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
