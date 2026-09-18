/** @vitest-environment jsdom */
// The scalar fields. The single field's commit cycle is story 14's core
// interactive behaviour — Enter or blur commits, Escape discards, an
// unchanged commit sends nothing, and a definition push re-seeds the
// draft — so it is pinned on the real component here; the same field,
// mixed, carries this story's rules: Enter is deliberate, leaving commits
// only an edit, and the mixed state marks the value area. What a
// committed text stores is the server's reading, tested server-side
// against the loader itself.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { commitParameter, Field, scalarPossible, scalarText } from './fields.jsx'

afterEach(cleanup)

function field() {
  return screen.getByRole('textbox')
}

describe('Field', () => {
  it('commits a changed draft on Enter', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.keyDown(field(), { key: 'Enter' })
    expect(onCommit).toHaveBeenCalledWith('9')
  })

  it('commits a changed draft on blur', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '2.5' } })
    fireEvent.blur(field())
    expect(onCommit).toHaveBeenCalledWith('2.5')
  })

  it('sends nothing while the draft is the stored value', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.keyDown(field(), { key: 'Enter' })
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('Escape discards the draft, and the next blur commits nothing', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: 'half-typed' } })
    fireEvent.keyDown(field(), { key: 'Escape' })
    expect(field().value).toBe('7')
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('a definition push re-seeds the draft, even mid-edit', () => {
    const onCommit = vi.fn()
    const { rerender } = render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    rerender(<Field value="42" onCommit={onCommit} />)
    expect(field().value).toBe('42')
  })

  it('Enter then blur before the push lands sends the same commit twice', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.keyDown(field(), { key: 'Enter' })
    fireEvent.blur(field())
    expect(onCommit).toHaveBeenCalledTimes(2)
    expect(onCommit).toHaveBeenNthCalledWith(2, '9')
  })

  it('Escape after an Enter commit returns to the stored value until the push re-seeds', () => {
    const onCommit = vi.fn()
    const { rerender } = render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.keyDown(field(), { key: 'Enter' })
    fireEvent.keyDown(field(), { key: 'Escape' })
    expect(field().value).toBe('7')
    rerender(<Field value="9" onCommit={onCommit} />)
    expect(field().value).toBe('9')
  })
})

describe('commitParameter', () => {
  it('commits the raw text — the server reads it as the file format does', () => {
    const edit = vi.fn()
    commitParameter(edit, 'u1', 'radius', '2.5')
    expect(edit).toHaveBeenCalledWith('set_parameter', {
      uuid: 'u1',
      input: 'radius',
      value: '2.5',
    })
  })

  it('empty text unsets — no value rides the message', () => {
    const edit = vi.fn()
    commitParameter(edit, 'u1', 'radius', '')
    expect(edit).toHaveBeenCalledWith('set_parameter', { uuid: 'u1', input: 'radius' })
  })
})

describe('scalarText', () => {
  it('renders each stored kind as its scalar text, no value as empty', () => {
    expect(scalarText(undefined)).toBe('')
    expect(scalarText(null)).toBe('')
    expect(scalarText(true)).toBe('true')
    expect(scalarText(false)).toBe('false')
    expect(scalarText(7)).toBe('7')
    expect(scalarText(2.5)).toBe('2.5')
    expect(scalarText('hi')).toBe('hi')
  })
})

describe('scalarPossible', () => {
  const baseScalars = { String: true, f64: true, 'alpha/ratio': false }

  it('a declared base scalar makes the port scalar-possible, whatever union', () => {
    expect(scalarPossible({ type_refs: ['String'] }, baseScalars)).toBe(true)
    expect(scalarPossible({ type_refs: ['alpha/ratio', 'f64'] }, baseScalars)).toBe(true)
  })

  it('a port declared only on custom types gets no field', () => {
    expect(scalarPossible({ type_refs: ['alpha/ratio'] }, baseScalars)).toBe(false)
  })

  it('a type reference the listing does not classify reads as not-a-base-scalar', () => {
    expect(scalarPossible({ type_refs: ['gone'] }, baseScalars)).toBe(false)
  })
})

describe('Field, mixed', () => {
  it('commits a typed value on Enter', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.keyDown(field(), { key: 'Enter' })
    expect(onCommit).toHaveBeenCalledWith('9')
  })

  it('commits a typed value on blur', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '2.5' } })
    fireEvent.blur(field())
    expect(onCommit).toHaveBeenCalledWith('2.5')
  })

  it('leaving without typing commits nothing, mixed or not', () => {
    const onCommit = vi.fn()
    const { rerender } = render(<Field value="7" onCommit={onCommit} />)
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
    rerender(<Field value="" mixed onCommit={onCommit} />)
    fireEvent.focus(field())
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('Enter on an untouched mixed field is the deliberate clear', () => {
    const onCommit = vi.fn()
    render(<Field value="" mixed onCommit={onCommit} />)
    expect(field().placeholder).toBe('mixed')
    fireEvent.keyDown(field(), { key: 'Enter' })
    expect(onCommit).toHaveBeenCalledWith('')
  })

  it('typed then cleared commits the unset on blur, from the mixed state', () => {
    const onCommit = vi.fn()
    render(<Field value="" mixed onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.change(field(), { target: { value: '' } })
    fireEvent.blur(field())
    expect(onCommit).toHaveBeenCalledWith('')
  })

  it('an edit that ends at the stored text commits nothing on blur', () => {
    const onCommit = vi.fn()
    render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.change(field(), { target: { value: '7' } })
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('Escape discards, and the next blur commits nothing even on a mixed field', () => {
    const onCommit = vi.fn()
    const { rerender } = render(<Field value="7" onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.keyDown(field(), { key: 'Escape' })
    expect(field().value).toBe('7')
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
    rerender(<Field value="" mixed onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    fireEvent.keyDown(field(), { key: 'Escape' })
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
  })

  it('a push re-seeds the field and clears the mixed marker', () => {
    const onCommit = vi.fn()
    const { rerender } = render(<Field value="" mixed onCommit={onCommit} />)
    fireEvent.change(field(), { target: { value: '9' } })
    rerender(<Field value="9" mixed={false} onCommit={onCommit} />)
    expect(field().value).toBe('9')
    expect(field().placeholder).toBe('')
    fireEvent.blur(field())
    expect(onCommit).not.toHaveBeenCalled()
  })
})
