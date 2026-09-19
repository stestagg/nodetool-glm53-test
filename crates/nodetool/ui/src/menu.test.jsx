/** @vitest-environment jsdom */
// The context menu: the entries the selection earns, the one way out per
// way in — a pick runs its action and closes, Escape and a press outside
// close quietly — and the items as buttons a keyboard can reach.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { ContextMenu } from './menu.jsx'

afterEach(cleanup)

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
})
