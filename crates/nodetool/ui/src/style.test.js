// What this stylesheet must not say. It loads after React Flow's own, so
// a declaration here silently outranks the library's — the port handle's
// connection cursor most of all, which this sheet once overrode with the
// `grab` of a node drag, making the wire's hot-zone read as a draggable
// surface.
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const css = readFileSync(new URL('./style.css', import.meta.url), 'utf8')

// The declarations one selector's block carries, as written.
function block(selector) {
  const at = css.indexOf(`\n${selector} {`)
  expect(at, `the stylesheet declares ${selector}`).toBeGreaterThan(-1)
  return css.slice(at, css.indexOf('}', at))
}

describe('the port handle’s cursor', () => {
  it('is left to React Flow, which gives a connectable handle its own', () => {
    expect(block('.port .react-flow__handle')).not.toMatch(/cursor:/)
  })

  it('is held over the origin of a wire in flight, where the library drops it', () => {
    expect(
      block('.app:not(.locked) .port .react-flow__handle.connectingfrom'),
    ).toMatch(/cursor:\s*crosshair/)
  })

  it('gives way to the default while editing is locked', () => {
    expect(block('.app.locked .react-flow__handle')).toMatch(/cursor:\s*default/)
  })
})
