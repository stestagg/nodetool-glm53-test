// The contrast floor, judged where the colours actually meet. The
// editor's own colours are read from the stylesheet's declarations — the
// theme as the UI defines it, no value restated here — and the base
// scalars' declared colours are read from the listing fact, pinned by the
// Rust side's own test, exactly as the server serves them. Text meets
// 4.5:1; large text and the meaningful edges — port dots, wires, the
// status and problem marks, the focus indicator, the banner — meet 3:1.
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { contrastRatio } from './contrast.js'
import { NEUTRAL } from './types.js'
import listingFact from '../listing-fact.json'

const css = readFileSync(new URL('./style.css', import.meta.url), 'utf8')

// The custom properties the stylesheet declares: the theme's colours, by
// name — the test reads what the editor renders, never a copy.
const vars = {}
for (const [, name, value] of css.matchAll(/(--[a-z-]+):\s*(#[0-9a-fA-F]{6})\s*;/g)) {
  vars[name] = value
}

const TEXT = 4.5
const EDGE = 3

function meets(ratio, floor, why) {
  expect(
    ratio,
    `${why}: ${ratio.toFixed(2)}:1, the floor is ${floor}:1`,
  ).toBeGreaterThanOrEqual(floor)
}

describe('the theme as the stylesheet defines it', () => {
  it('declares the colours the floor is judged against', () => {
    for (const name of [
      '--text',
      '--muted',
      '--panel',
      '--canvas',
      '--titlebar',
      '--accent',
      '--danger',
      '--report-text',
    ]) {
      expect(vars[name], name).toMatch(/^#[0-9a-fA-F]{6}$/)
    }
  })

  it('body and label text clears 4.5:1 on every panel it sits on', () => {
    for (const surface of ['--panel', '--canvas', '--titlebar']) {
      meets(contrastRatio(vars['--text'], vars[surface]), TEXT, `text on ${surface}`)
      meets(contrastRatio(vars['--muted'], vars[surface]), TEXT, `muted text on ${surface}`)
    }
  })

  it('the accent and the danger read as text where they are text', () => {
    meets(contrastRatio(vars['--accent'], vars['--panel']), TEXT, 'accent text on the panel')
    meets(contrastRatio(vars['--danger'], vars['--panel']), TEXT, 'problem text on the panel')
  })

  it('the reports stay readable on the danger they are painted on', () => {
    meets(contrastRatio(vars['--report-text'], vars['--danger']), TEXT, 'banner and toast text')
  })

  it('the derived statuses clear 4.5:1 on the title bar they name', () => {
    for (const name of ['--status-running', '--status-completed', '--status-failed']) {
      meets(contrastRatio(vars[name], vars['--titlebar']), TEXT, `${name} status`)
    }
  })

  it('the focus indicator, marks, and banner stand out as edges, 3:1', () => {
    for (const surface of ['--panel', '--canvas', '--titlebar']) {
      meets(contrastRatio(vars['--accent'], vars[surface]), EDGE, `focus ring on ${surface}`)
    }
    for (const surface of ['--panel', '--canvas', '--titlebar']) {
      meets(contrastRatio(vars['--danger'], vars[surface]), EDGE, `problem mark on ${surface}`)
    }
  })

  it('a port’s focus ring stands outside the dot, panel colour beneath the accent', () => {
    // The pairing that matters on a port is ring-against-panel — inside
    // the dot the accent would meet the dot's own declared colour, 1:1 on
    // the numeric ports. The rule itself is the claim, so a revert to an
    // in-dot ring fails here rather than passing silently.
    const rule = css.match(/\.port \.react-flow__handle[^{}]*:focus-visible\s*\{([^}]*)\}/)
    expect(rule, 'the port declares its own focus treatment').toBeTruthy()
    expect(rule[1]).toContain('outline: none')
    expect(rule[1]).toContain('var(--panel)')
    expect(rule[1]).toContain('var(--accent)')
  })
})

describe('the neutral pair, the browser side of the one neutral', () => {
  it('is the fact the server composes', () => {
    expect(listingFact['beta/ghost-type'].color).toBe(NEUTRAL.color)
    expect(listingFact['beta/ghost-type'].shape).toBe(NEUTRAL.shape)
  })

  it('clears 3:1 as a port dot on the node and as a wire on the canvas', () => {
    meets(contrastRatio(NEUTRAL.color, vars['--panel']), EDGE, 'neutral port dot')
    meets(contrastRatio(NEUTRAL.color, vars['--canvas']), EDGE, 'neutral wire')
  })
})

describe('the base scalars’ declared colours, read from the listing fact', () => {
  const scalars = Object.keys(listingFact)
    .filter((name) => !name.includes('/'))
    .map((name) => ({ name, color: listingFact[name].color }))

  it('every base scalar declares a colour, riding the listing fact', () => {
    expect(scalars.length).toBeGreaterThanOrEqual(12)
    for (const scalar of scalars) {
      expect(scalar.color, scalar.name).toMatch(/^#[0-9a-fA-F]{6}$/)
    }
  })

  it('each clears 3:1 as a port dot on the node’s surface and a wire on the canvas', () => {
    for (const scalar of scalars) {
      meets(contrastRatio(scalar.color, vars['--panel']), EDGE, `${scalar.name} port dot`)
      meets(contrastRatio(scalar.color, vars['--canvas']), EDGE, `${scalar.name} wire`)
    }
  })
})
