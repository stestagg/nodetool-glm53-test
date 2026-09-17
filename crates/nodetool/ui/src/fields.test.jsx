// The scalar text the two views of an input's value share: how a stored
// scalar renders into a field and how a committed field's text reads
// back as the plain scalar the file format would carry. Pure functions —
// the visible proof exercises the field's commit cycle by hand.
import { describe, expect, it } from 'vitest'
import { parseScalarText, scalarPossible, scalarText } from './fields.jsx'

describe('scalarText', () => {
  it('renders each stored kind as its scalar text, no value as empty', () => {
    expect(scalarText(undefined)).toBe('')
    expect(scalarText(true)).toBe('true')
    expect(scalarText(false)).toBe('false')
    expect(scalarText(7)).toBe('7')
    expect(scalarText(2.5)).toBe('2.5')
    expect(scalarText('hi')).toBe('hi')
  })
})

describe('parseScalarText', () => {
  it('reads a committed field as the file format reads its text', () => {
    expect(parseScalarText('true')).toBe(true)
    expect(parseScalarText('false')).toBe(false)
    expect(parseScalarText('7')).toBe(7)
    expect(parseScalarText('-3')).toBe(-3)
    expect(parseScalarText('2.5')).toBe(2.5)
    expect(parseScalarText('-0.5')).toBe(-0.5)
    expect(parseScalarText('1e3')).toBe(1000)
    expect(parseScalarText('hi')).toBe('hi')
    expect(parseScalarText('2.5 kg')).toBe('2.5 kg')
    expect(parseScalarText('True')).toBe('True')
    expect(parseScalarText('')).toBe('')
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
