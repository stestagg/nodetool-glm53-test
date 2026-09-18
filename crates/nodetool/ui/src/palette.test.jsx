/** @vitest-environment jsdom */
// The palette: the grouping the palette rows render, and the rows' two
// ways of creating a node — the drag the canvas reads, and the keyboard
// activation the shell turns into the same create a drop sends. While
// the lock holds, a row keeps its place but goes quiet with its drag.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { Palette, paletteGroups } from './palette.jsx'

afterEach(cleanup)

const types = [
  { type_ref: 'text/split', label: 'Split', plugin: 'text', sub_group: null },
  { type_ref: 'text/check', label: 'Check', plugin: 'text', sub_group: null },
  { type_ref: 'shapes/sphere', label: 'Sphere', plugin: 'shapes', sub_group: '3d' },
  { type_ref: 'shapes/polygon', label: 'Polygon', plugin: 'shapes', sub_group: '2d' },
  { type_ref: 'shapes/circle', label: 'Circle', plugin: 'shapes', sub_group: '2d' },
  { type_ref: 'shapes/rectangle', label: 'Rectangle', plugin: 'shapes', sub_group: '2d' },
  { type_ref: 'alpha/mix', label: 'Mix', plugin: 'alpha', sub_group: null },
  { type_ref: 'alpha/add', label: 'Add', plugin: 'alpha', sub_group: 'math' },
  { type_ref: 'alpha/concat', label: 'Concat', plugin: 'alpha', sub_group: 'text' },
]

describe('paletteGroups', () => {
  it('groups by plugin and sub-group, plugins and sub-groups alphabetical', () => {
    const groups = paletteGroups(types)
    expect(groups.map((group) => group.plugin)).toEqual(['alpha', 'shapes', 'text'])
    expect(groups[0].sections.map((section) => section.subGroup)).toEqual([null, 'math', 'text'])
    expect(groups[1].sections.map((section) => section.subGroup)).toEqual(['2d', '3d'])
  })

  it('types order alphabetically within their group', () => {
    const [, shapes] = paletteGroups(types)
    const [twoD, threeD] = shapes.sections
    expect(twoD.types.map((type) => type.label)).toEqual(['Circle', 'Polygon', 'Rectangle'])
    expect(threeD.types.map((type) => type.label)).toEqual(['Sphere'])
  })

  it('types without a sub-group sit directly under the plugin header, first', () => {
    const [alpha] = paletteGroups(types)
    expect(alpha.sections[0].subGroup).toBeNull()
    expect(alpha.sections[0].types.map((type) => type.label)).toEqual(['Mix'])
  })

  it('a flat plugin has no sub-sections', () => {
    const [, , text] = paletteGroups(types)
    expect(text.sections).toEqual([
      {
        subGroup: null,
        types: [
          expect.objectContaining({ label: 'Check' }),
          expect.objectContaining({ label: 'Split' }),
        ],
      },
    ])
  })

  it('every reload and tab reads the same sections', () => {
    expect(paletteGroups(types)).toEqual(paletteGroups([...types].reverse()))
  })

  it('a label tie orders by type reference, deterministically', () => {
    const tied = [
      { type_ref: 'zeta/twin', label: 'Twin', plugin: 'zeta', sub_group: null },
      { type_ref: 'alpha/twin', label: 'Twin', plugin: 'zeta', sub_group: null },
    ]
    expect(paletteGroups(tied)[0].sections[0].types.map((type) => type.type_ref)).toEqual([
      'alpha/twin',
      'zeta/twin',
    ])
    expect(paletteGroups([...tied].reverse())).toEqual(paletteGroups(tied))
  })
})

describe('Palette rows', () => {
  const palette = (editable, onCreate) =>
    render(<Palette types={types} editable={editable} onCreate={onCreate} />)

  it('an activated row creates through the same create a drop sends', () => {
    const onCreate = vi.fn()
    palette(true, onCreate)
    fireEvent.keyDown(screen.getByText('Add'), { key: 'Enter' })
    expect(onCreate).toHaveBeenCalledWith('alpha/add')
    fireEvent.keyDown(screen.getByText('Add'), { key: ' ' })
    expect(onCreate).toHaveBeenCalledTimes(2)
  })

  it('another key does nothing, and the row carries the type reference a drop reads', () => {
    const onCreate = vi.fn()
    palette(true, onCreate)
    fireEvent.keyDown(screen.getByText('Add'), { key: 'a' })
    expect(onCreate).not.toHaveBeenCalled()
    expect(screen.getByText('Add').closest('li').tabIndex).toBe(0)
  })

  it('the lock quiets the row with its drag: no activation, no tab stop', () => {
    const onCreate = vi.fn()
    palette(false, onCreate)
    fireEvent.keyDown(screen.getByText('Add'), { key: 'Enter' })
    expect(onCreate).not.toHaveBeenCalled()
    expect(screen.getByText('Add').closest('li').tabIndex).toBe(-1)
  })
})
