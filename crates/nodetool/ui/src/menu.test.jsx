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
})
