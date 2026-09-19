// The editor's context menu: the selection's gestures where the pointer
// already is, and — for the canvas background — the document's groups.
// The shell opens it on a right-click — on a node in the selection, or on
// an unselected node it selects alone first — and the menu acts on the
// selection it opened for: opening never disturbs it. It closes on a
// pick, on Escape, and on a press outside it. Its items are buttons in
// the tab order, and Enter or Space activates the row its click does —
// the palette row's rule, the keyboard that started here finishing here
// without leaning on the browser's button activation. An entry may carry
// a submenu: the parent row opens it, its panel seating itself beside the
// row clamped inside the canvas, its first row taking focus. The chords
// (keyboard.js's groupKeys) issue the same operations without it. Seated
// where the pointer is, clamped to stay inside the canvas — a right-click
// near an edge opens a menu the pointer can still reach.

import { useEffect, useLayoutEffect, useRef, useState } from 'react'

// Enter or Space — the two activation keys, whichever the hand came from.
function activated(event) {
  return event.key === 'Enter' || event.key === ' '
}

// One menu row: the pick runs on a click and on its activation keys —
// the same operation either hand sends, the key press prevented so the
// browser's own activation cannot double it.
function Item({ entry, onClose }) {
  const pick = () => {
    entry.onPick()
    onClose()
  }
  return (
    <button
      role="menuitem"
      onClick={pick}
      onKeyDown={(event) => {
        if (activated(event)) {
          event.preventDefault()
          pick()
        }
      }}
    >
      {entry.label}
    </button>
  )
}

// A submenu: the parent row opens and closes the panel, the panel seats
// itself beside the row — clamped inside the canvas the menu renders in,
// the reach the menu itself keeps — and its first row takes focus, the
// keyboard's way in. The panel is inside the menu div, so the outside
// press and Escape the menu listens for cover it.
function Submenu({ entry, onClose }) {
  const row = useRef(null)
  const panel = useRef(null)
  const [open, setOpen] = useState(false)
  useLayoutEffect(() => {
    if (!open) return
    const node = panel.current
    const menu = node?.closest('.menu')
    const canvas = menu?.parentElement
    if (node === null || menu === null || canvas === null) return
    node.style.left = `${menu.offsetWidth}px`
    node.style.top = `${Math.max(
      0,
      Math.min(row.current.offsetTop, canvas.clientHeight - menu.offsetTop - node.offsetHeight),
    )}px`
    node.querySelector('button')?.focus()
  }, [open])
  const toggle = () => setOpen((current) => !current)
  return (
    <div>
      <button
        ref={row}
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={toggle}
        onKeyDown={(event) => {
          if (activated(event)) {
            event.preventDefault()
            toggle()
          }
        }}
      >
        {entry.label}
      </button>
      {open && (
        <div className="menu submenu" role="menu" ref={panel}>
          {entry.items.map((item) => (
            <Item key={item.key} entry={item} onClose={onClose} />
          ))}
        </div>
      )}
    </div>
  )
}

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
      {entries.map((entry) =>
        entry.items === undefined ? (
          <Item key={entry.key} entry={entry} onClose={onClose} />
        ) : (
          <Submenu key={entry.key} entry={entry} onClose={onClose} />
        ),
      )}
    </div>
  )
}
