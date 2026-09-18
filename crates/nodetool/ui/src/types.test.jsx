// The type channel's reading: what a port renders from the listing's
// data-type fact. The single-declared, union-declared, unknown-reference,
// unknown-shape, and not-yet-listed cases are the whole rule, so they are
// pinned here; the server's composition of the fact is the Rust suite's.
import { describe, expect, it } from 'vitest'
import { NEUTRAL, portAppearance } from './types.js'

describe('NEUTRAL', () => {
  it('is the pair the server composes, pinned as a literal against drift', () => {
    expect(NEUTRAL).toEqual({ color: '#8f99a8', shape: 'circle' })
  })
})

describe('portAppearance', () => {
  const dataTypes = {
    String: { color: '#238551', shape: 'circle' },
    i64: { color: '#2d72d2', shape: 'square' },
    'alpha/ratio': { color: '#0e9488', shape: 'hexagon' },
  }

  it('a port declaring exactly one type renders in that type’s colour and shape', () => {
    expect(portAppearance({ type_refs: ['String'] }, dataTypes)).toEqual({
      color: '#238551',
      shape: 'circle',
    })
    expect(portAppearance({ type_refs: ['i64'] }, dataTypes)).toEqual({
      color: '#2d72d2',
      shape: 'square',
    })
  })

  it('a union-declared port has no single type to inform and takes the neutral', () => {
    expect(portAppearance({ type_refs: ['i32', 'f64'] }, dataTypes)).toEqual(NEUTRAL)
  })

  it('a reference the listing holds no fact for takes the neutral', () => {
    expect(portAppearance({ type_refs: ['gone/missing'] }, dataTypes)).toEqual(NEUTRAL)
  })

  it('a port nothing declares — a wire’s source the listing cannot produce — takes the neutral', () => {
    expect(portAppearance(undefined, dataTypes)).toEqual(NEUTRAL)
  })

  it('a declared shape outside the drawn set takes the neutral shape, the colour verbatim', () => {
    expect(portAppearance({ type_refs: ['alpha/ratio'] }, dataTypes)).toEqual({
      color: '#0e9488',
      shape: 'circle',
    })
  })

  it('a listing not yet arrived takes the neutral', () => {
    expect(portAppearance({ type_refs: ['String'] })).toEqual(NEUTRAL)
  })
})
