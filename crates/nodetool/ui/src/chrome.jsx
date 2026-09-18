// The chrome's report surfaces. Toasts are the one surface a transient
// report arrives through — dismissed on click and on their own, never
// the durable truth — and the region they arrive through is a polite
// live region, present whenever the editor is, so a report is announced
// as it appears to assistive technology and takes no focus doing it.
// The banner is the same promise for the connection's state: the loss
// names itself in a polite live region over the last-known canvas.

export function Toasts({ toasts, onDismiss }) {
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className="toast" onClick={() => onDismiss(toast.id)}>
          {toast.text}
        </div>
      ))}
    </div>
  )
}

export function Banner({ text }) {
  return (
    <div className="banner" role="status" aria-live="polite">
      {text}
    </div>
  )
}
