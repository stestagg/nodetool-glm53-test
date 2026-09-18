// The Shape stage's node UI — the plugin's own rendering of the body
// between the editor's title bar and ports. Built against plugin UI
// contract version 1: the component receives the node's definition slice
// and the live display state, and issues parameter edits through the
// helper the contract hands it, the same operation the sidebar and the
// default node's inline fields commit. The editor's title bar already
// carries the label, icon, and status, and the ports the readouts, so the
// body holds only what they do not: the staged value and the note's own
// field. Its styling rides with it — the editor hardcodes nothing
// plugin-specific.

const BLUE = '#4a90d9'
// The value chip's ink: dark enough to carry white text at the contract's
// accessibility floor; the lighter blue stays on the decoration.
const INK = '#2d5fa8'

export default function StageNode({ h, parameters, portValues, marks, locked, setParameter }) {
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
    h('output', {
      style: {
        minWidth: '72px',
        padding: '3px 8px',
        borderRadius: '3px',
        background: INK,
        color: 'white',
        fontWeight: 600,
        fontSize: '13px',
        textAlign: 'center',
      },
    }, portValues.shape !== undefined ? portValues.shape : '\u2014'),
    // The field follows the stored parameter: `key` remounts it when a
    // change lands from another surface, so it never holds stale text —
    // and a blur commits only what the user actually typed here.
    h('input', {
      key: note,
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
