// The type channel: the colour and shape a port or wire renders from the
// listing's data-type fact. A port declaring exactly one type renders in
// that type's fact — the colour verbatim, the shape from the small closed
// set this renderer draws. A union-declared port, a reference the listing
// holds no fact for, and a listing not yet arrived take the neutral pair —
// the browser's own, the same pair the server composes for a type that
// does not declare both, so there is one neutral everywhere. A declared
// shape outside the drawn set takes the neutral shape, the colour
// verbatim: the declarer's to fix, as with icons.

export const NEUTRAL = { color: '#8f99a8', shape: 'circle' }

const SHAPES = new Set(['circle', 'square'])

// What a port renders: its single declared type's appearance, or the
// neutral — a port no type declares (a wire's source the listing cannot
// produce) reads as the neutral too. `dataTypes` is the listing's
// colour-and-shape fact, keyed by type reference.
export function portAppearance(port, dataTypes = {}) {
  const fact =
    port?.type_refs.length === 1 ? dataTypes[port.type_refs[0]] : undefined
  if (!fact) return NEUTRAL
  return {
    color: fact.color,
    shape: SHAPES.has(fact.shape) ? fact.shape : NEUTRAL.shape,
  }
}
