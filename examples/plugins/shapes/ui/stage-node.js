// The Shape stage's node UI — the plugin's own rendering of the body
// between the editor's title bar and ports. Built against plugin UI
// contract version 1: the component receives the node's definition slice
// and the live display state, and issues label and parameter edits through
// the two helpers the contract hands it, the same operations the sidebar
// and the default node's inline fields commit. Its styling rides with it —
// the editor hardcodes nothing plugin-specific.

const BLUE = '#4a90d9'

export default function StageNode({
  h,
  label,
  parameters,
  status,
  portValues,
  marks,
  locked,
  setParameter,
}) {
  const note = typeof parameters.note === 'string' ? parameters.note : ''
  return h('div', { style: { display: 'grid', gap: '4px', justifyItems: 'center', maxWidth: '180px' } },
    h('img', {
      src: 'data:image/svg+xml;utf8,' + encodeURIComponent(
        '<svg xmlns="http://www.w3.org/2000/svg" width="26" height="26">' +
        '<circle cx="13" cy="13" r="9" fill="none" stroke="' + BLUE + '" stroke-width="2.5"/>' +
        '</svg>',
      ),
      alt: '',
      draggable: false,
      width: 26,
      height: 26,
    }),
    h('div', { style: { display: 'flex', gap: '6px', alignItems: 'baseline' } },
      h('strong', { style: { fontSize: '11px' } }, label),
      status !== undefined && h('span', { style: { fontSize: '9px', color: '#6a7383' } }, status),
    ),
    h('output', {
      style: {
        minWidth: '72px',
        padding: '3px 8px',
        borderRadius: '3px',
        background: BLUE,
        color: 'white',
        fontWeight: 600,
        textAlign: 'center',
      },
    }, portValues.shape !== undefined ? portValues.shape : '\u2014'),
    note !== '' && h('p', { style: { margin: 0, fontSize: '11px', color: '#6a7383' } }, note),
    h('input', {
      className: 'field nodrag',
      'aria-label': 'note',
      placeholder: 'caption\u2026',
      defaultValue: note,
      disabled: locked,
      style: { width: '100%', textAlign: 'center' },
      onKeyDown: (event) => {
        if (event.key === 'Enter') setParameter('note', event.target.value)
      },
      onBlur: (event) => {
        if (event.target.value !== note) setParameter('note', event.target.value)
      },
    }),
    marks.length > 0 && h(
      'div',
      { style: { color: '#a83a3e', fontSize: '10px', whiteSpace: 'pre-wrap', width: '100%' } },
      marks.join('\n'),
    ),
  )
}
