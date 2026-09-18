// @vitest-environment jsdom
// The coalescer is the one place the canvas's rendering promise lives:
// latest value per port, frames dropped instead of queued, pulses that
// gather a fan-out and go quiet one beat after the stream stops, the
// reduced-motion preference quieting the pulses and not the values. The
// visible proof exercises it by hand once; these tests pin it.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PULSE_MS, RunCoalescer } from './coalesce.js'

beforeEach(() => {
  vi.useFakeTimers({
    toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame', 'cancelAnimationFrame'],
  })
  vi.spyOn(window, 'matchMedia').mockReturnValue({ matches: false })
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

// One animation frame's tick.
const flushFrame = () => vi.advanceTimersByTime(16)

function setup() {
  const applied = []
  return { applied, coalescer: new RunCoalescer((view) => applied.push(view)) }
}

describe('RunCoalescer', () => {
  it('a frame keeps only the latest value per port', () => {
    const { applied, coalescer } = setup()
    coalescer.emitted('a', 'out', '1', [])
    coalescer.emitted('a', 'out', '2', [])
    expect(applied).toEqual([])
    flushFrame()
    expect(applied).toEqual([{ values: { 'a/out': '2' }, pulsing: new Set() }])
  })

  it('a later frame lands over what the canvas held, never under it', () => {
    const { applied, coalescer } = setup()
    coalescer.emitted('a', 'out', '1', [])
    flushFrame()
    coalescer.emitted('a', 'out', '2', [])
    coalescer.emitted('b', 'out', '9', [])
    flushFrame()
    expect(applied.at(-1).values).toEqual({ 'a/out': '2', 'b/out': '9' })
  })

  it('a burst of emissions animates once, at the frame', () => {
    const { applied, coalescer } = setup()
    for (let value = 1; value <= 50; value += 1) coalescer.emitted('a', 'out', String(value), [])
    expect(applied).toEqual([])
    flushFrame()
    expect(applied).toEqual([{ values: { 'a/out': '50' }, pulsing: new Set() }])
  })

  it('a fan-out pulses each wire it travels and goes quiet one beat after the stream stops', () => {
    const { applied, coalescer } = setup()
    coalescer.emitted('a', 'out', '1', ['a/out->b/in', 'a/out->c/in'])
    flushFrame()
    expect(applied.at(-1).pulsing).toEqual(new Set(['a/out->b/in', 'a/out->c/in']))
    coalescer.emitted('a', 'out', '2', ['a/out->d/value'])
    flushFrame()
    expect(applied.at(-1).pulsing).toEqual(
      new Set(['a/out->b/in', 'a/out->c/in', 'a/out->d/value']),
    )
    vi.advanceTimersByTime(PULSE_MS - 1)
    expect(applied).toHaveLength(2)
    vi.advanceTimersByTime(1)
    expect(applied.at(-1).pulsing).toEqual(new Set())
  })

  it('the reduced-motion preference quiets the pulses and leaves the values flowing', () => {
    window.matchMedia.mockReturnValue({ matches: true })
    const { applied, coalescer } = setup()
    coalescer.emitted('a', 'out', '1', ['a/out->b/in'])
    flushFrame()
    expect(window.matchMedia).toHaveBeenCalledWith('(prefers-reduced-motion: reduce)')
    expect(applied).toEqual([{ values: { 'a/out': '1' }, pulsing: new Set() }])
  })

  it('a custom-typed emission pulses the wire and holds no value', () => {
    const { applied, coalescer } = setup()
    coalescer.emitted('a', 'mark', undefined, ['a/mark->b/in'])
    flushFrame()
    expect(applied).toEqual([{ values: {}, pulsing: new Set(['a/mark->b/in']) }])
  })

  it('the connect-time display replaces what is held outright, pending included', () => {
    const { applied, coalescer } = setup()
    coalescer.emitted('a', 'out', '1', ['a/out->b/in'])
    coalescer.replace({ 'r/out': '9' })
    flushFrame()
    expect(applied.at(-1).values).toEqual({ 'r/out': '9' })
  })
})
