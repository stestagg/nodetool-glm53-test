// The shape value's display, as the type-value UI contract version 1
// renders it: the component receives the value in the form the plugin's
// serialiser produced and presents it wherever shape values display — the
// port readouts first among them. Its styling rides with it, the way the
// icons do.

const BLUE = '#4a90d9'

export default function ShapeValue({ h, value }) {
  return h(
    'span',
    {
      style: {
        padding: '0 4px',
        borderRadius: '2px',
        background: BLUE,
        color: 'white',
        fontWeight: 600,
      },
    },
    value,
  )
}
