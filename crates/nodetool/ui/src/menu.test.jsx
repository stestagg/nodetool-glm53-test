/** @vitest-environment jsdom */
// The context menu: the entries the selection earns, the one way out per
// way in — a pick runs its action and closes, Escape and a press outside
// close quietly — and the items as buttons a keyboard can reach, the
// browser turning Enter or Space into the click. An entry may carry a
// submenu: the parent row opens it, its rows pick and close, Escape and
// the outside press reach it as they reach the menu itself, and its
// panel clamps inside the canvas as the menu does.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ContextMenu } from './menu.jsx'

afterEach(() => {
  cleanup()
  vi.clearAllMocks()
})

const GROUPS = [
  {
    key: 'groups',
    label: 'Groups',
    items: [
      { key: 'filter', label: 'filter', onPick: vi.fn() },
      { key: 'stage', label: 'stage', onPick: vi.fn() },
    ],
  },
]

function menu(entries = null, onClose = vi.fn()) {
  render(
    <ContextMenu
      x={12}
      y={30}
      entries={
        entries ?? [
          { key: 'package', label: 'Package into group…', onPick: vi.fn() },
          { key: 'unpack', label: 'Unpack group', onPick: vi.fn() },
        ]
      }
      onClose={onClose}
    />,
  )
  return { onClose }
}

describe('ContextMenu', () => {
  it('shows the entries the selection earns, as menu items', () => {
    menu()
    expect(screen.getByRole('menu')).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: 'Package into group…' })).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: 'Unpack group' })).toBeTruthy()
  })

  it('a pick runs its action and closes', () => {
    const onPick = vi.fn()
    const { onClose } = menu([{ key: 'unpack', label: 'Unpack group', onPick }])
    fireEvent.click(screen.getByRole('menuitem', { name: 'Unpack group' }))
    expect(onPick).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('a row is a button, so the keyboard finishes in it as the click does', () => {
    menu()
    expect(screen.getByRole('menuitem', { name: 'Unpack group' }).tagName).toBe('BUTTON')
  })

  it('a submenu parent opens and closes its panel; opening focuses its first row', () => {
    menu(GROUPS)
    expect(screen.queryByRole('menuitem', { name: 'filter' })).toBeNull()
    fireEvent.click(screen.getByRole('menuitem', { name: 'Groups' }))
    expect(screen.getByRole('menuitem', { name: 'filter' })).toBeTruthy()
    expect(screen.getByRole('menuitem', { name: 'stage' })).toBeTruthy()
    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'filter' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'Groups' }))
    expect(screen.queryByRole('menuitem', { name: 'filter' })).toBeNull()
  })

  it('a pick in the panel runs its action and closes the whole menu', () => {
    const { onClose } = menu(GROUPS)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Groups' }))
    fireEvent.click(screen.getByRole('menuitem', { name: 'stage' }))
    expect(GROUPS[0].items[1].onPick).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('a press inside the open submenu keeps the menu, Escape and a press outside close it', () => {
    const { onClose } = menu(GROUPS)
    fireEvent.click(screen.getByRole('menuitem', { name: 'Groups' }))
    fireEvent.mouseDown(screen.getByRole('menuitem', { name: 'filter' }))
    expect(onClose).not.toHaveBeenCalled()
    fireEvent.mouseDown(document.body)
    expect(onClose).toHaveBeenCalledTimes(1)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('Escape closes quietly, the selection it acted on untouched', () => {
    const { onClose } = menu()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('a press outside closes; a press on the menu does not', () => {
    const { onClose } = menu()
    fireEvent.mouseDown(screen.getByRole('menu'))
    expect(onClose).not.toHaveBeenCalled()
    fireEvent.mouseDown(document.body)
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('a seat near the canvas edge clamps back inside; a fitting seat keeps its place', () => {
    const sized = (owner, property, value) =>
      Object.defineProperty(owner, property, { configurable: true, value })
    sized(HTMLElement.prototype, 'offsetWidth', 170)
    sized(HTMLElement.prototype, 'offsetHeight', 76)
    try {
      const seat = (x, y) => {
        const canvas = document.body.appendChild(document.createElement('div'))
        sized(canvas, 'clientWidth', 400)
        sized(canvas, 'clientHeight', 200)
        render(
          <ContextMenu
            x={x}
            y={y}
            entries={[{ key: 'package', label: 'Package into group…', onPick: vi.fn() }]}
            onClose={vi.fn()}
          />,
          { container: canvas },
        )
        return canvas.firstElementChild
      }
      expect(seat(350, 180).style.left).toBe('230px')
      expect(seat(350, 180).style.top).toBe('124px')
      expect(seat(12, 30).style.left).toBe('12px')
      expect(seat(12, 30).style.top).toBe('30px')
    } finally {
      delete HTMLElement.prototype.offsetWidth
      delete HTMLElement.prototype.offsetHeight
    }
  })

  it('a submenu past the canvas right edge flips to the menu left; a fitting one sits at its right', () => {
    const sized = (owner, property, value) =>
      Object.defineProperty(owner, property, { configurable: true, value })
    sized(HTMLElement.prototype, 'offsetWidth', 170)
    sized(HTMLElement.prototype, 'offsetHeight', 76)
    sized(HTMLElement.prototype, 'offsetTop', 30)
    try {
      const panel = (menuOffsetLeft) => {
        const canvas = document.body.appendChild(document.createElement('div'))
        sized(canvas, 'clientWidth', 400)
        sized(canvas, 'clientHeight', 200)
        const view = render(<ContextMenu x={12} y={30} entries={GROUPS} onClose={vi.fn()} />, {
          container: canvas,
        })
        // The submenu seats itself when its row opens it, measuring the
        // menu's real place; the stub decides where that place is.
        sized(canvas.querySelector('.menu'), 'offsetLeft', menuOffsetLeft)
        fireEvent.click(screen.getByRole('menuitem', { name: 'Groups' }))
        const left = screen.getByRole('menuitem', { name: 'filter' }).parentElement.style.left
        view.unmount()
        canvas.remove()
        return left
      }
      expect(panel(240)).toBe('-170px')
      expect(panel(12)).toBe('170px')
    } finally {
      delete HTMLElement.prototype.offsetWidth
      delete HTMLElement.prototype.offsetHeight
      delete HTMLElement.prototype.offsetTop
    }
  })
})
