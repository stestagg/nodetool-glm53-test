/** @vitest-environment jsdom */
// The chrome's report surfaces: the toast region and the banner are the
// polite live regions every transient report and connection state
// arrives through — present whenever the editor is, announcing as they
// appear, and never taking focus to do it.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Banner, Toasts } from './chrome.jsx'

afterEach(cleanup)

describe('Toasts', () => {
  it('reports arrive in a live region that is present, polite, and focus-free', () => {
    render(<Toasts toasts={[{ id: 1, text: 'the run failed' }]} onDismiss={() => {}} />)
    const region = screen.getByRole('status')
    expect(region.getAttribute('aria-live')).toBe('polite')
    expect(region.textContent).toContain('the run failed')
    expect(region.querySelectorAll('[tabindex]')).toHaveLength(0)
    expect(document.activeElement).not.toBe(region)
  })

  it('the region exists with no report to carry, so the next one is announced', () => {
    render(<Toasts toasts={[]} onDismiss={() => {}} />)
    expect(screen.getByRole('status').getAttribute('aria-live')).toBe('polite')
  })

  it('a toast dismisses on click, the pointer affordance it already had', () => {
    const onDismiss = vi.fn()
    render(<Toasts toasts={[{ id: 3, text: 'save failed: denied' }]} onDismiss={onDismiss} />)
    fireEvent.click(screen.getByText('save failed: denied'))
    expect(onDismiss).toHaveBeenCalledWith(3)
  })
})

describe('Banner', () => {
  it('the connection state announces politely, without taking focus', () => {
    render(<Banner text="connection lost — trying to reconnect…" />)
    const region = screen.getByRole('status')
    expect(region.getAttribute('aria-live')).toBe('polite')
    expect(region.textContent).toContain('connection lost — trying to reconnect…')
    expect(region.querySelectorAll('[tabindex]')).toHaveLength(0)
  })

  it('no connection state leaves the region in place, carrying nothing', () => {
    render(<Banner text={null} />)
    expect(screen.getByRole('status').textContent).toBe('')
  })
})
