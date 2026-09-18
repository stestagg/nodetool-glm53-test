// The WCAG contrast reading the editor's colours are judged by: relative
// luminance over sRGB, and the ratio between two colours. The contrast
// test owns which pairs are judged and against what surfaces; this is
// only the arithmetic, stated once.

// A hex colour's relative luminance, the WCAG 2.x reading.
export function luminance(hex) {
  const channels = [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16) / 255)
  const [r, g, b] = channels.map((channel) =>
    channel <= 0.03928 ? channel / 12.92 : Math.pow((channel + 0.055) / 1.055, 2.4),
  )
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

// The contrast ratio between two colours, the larger over the smaller,
// from 1 (indistinguishable) to 21 (black on white).
export function contrastRatio(a, b) {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (light + 0.05) / (dark + 0.05)
}
