// The emission coalescer: one animation frame gathers the latest value
// per port and the wires an emission crossed, so a fast graph animates by
// dropping frames instead of queueing a backlog — a later emission
// overwrites an earlier one within the frame and the frame's merge lands
// over what the canvas held, never the reverse. What the canvas shows is
// always the events' own. A wire stays pulsing one beat past the last
// frame the coalescing flushed, so a steady stream reads as a flow and a
// stopped one goes quiet; the platform's reduced-motion preference quiets
// the pulses, the values not being motion.

// How long a wire keeps its pulse after an emission.
export const PULSE_MS = 300

export class RunCoalescer {
  // `apply` receives every change as `{ values, pulsing }` — the values
  // the canvas shows and the set of wires animating.
  constructor(apply, pulseMs = PULSE_MS) {
    this.apply = apply
    this.pulseMs = pulseMs
    this.values = {}
    this.pulsing = new Set()
    this.pendingValues = null
    this.pendingPulses = null
    this.frame = null
    this.quiet = null
  }

  // One forwarded emission: the value becomes the port's pending one —
  // the latest wins — and the wires the value travels join the frame's
  // pulse set. A custom-typed emission carries no value and still pulses.
  emitted(node, port, value, targets) {
    if (value !== undefined) {
      ;(this.pendingValues ??= {})[`${node}/${port}`] = value
    }
    for (const id of targets) {
      ;(this.pendingPulses ??= new Set()).add(id)
    }
    if (this.pendingValues !== null || this.pendingPulses !== null) {
      this.frame ??= requestAnimationFrame(() => this.flush())
    }
  }

  // The connect-time display and the canvas reset replace what is held
  // outright — nothing pending from an earlier state outlives them.
  replace(values) {
    this.values = { ...values }
    this.pendingValues = null
    this.pendingPulses = null
    this.apply({ values: this.values, pulsing: this.pulsing })
  }

  flush() {
    this.frame = null
    let changed = false
    if (this.pendingValues !== null) {
      this.values = { ...this.values, ...this.pendingValues }
      this.pendingValues = null
      changed = true
    }
    if (this.pendingPulses !== null && this.pendingPulses.size > 0) {
      const incoming = this.pendingPulses
      this.pendingPulses = null
      if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
        this.pulsing = new Set([...this.pulsing, ...incoming])
        clearTimeout(this.quiet)
        this.quiet = setTimeout(() => {
          this.pulsing = new Set()
          this.apply({ values: this.values, pulsing: this.pulsing })
        }, this.pulseMs)
        changed = true
      }
    }
    if (changed) this.apply({ values: this.values, pulsing: this.pulsing })
  }

  dispose() {
    clearTimeout(this.quiet)
    if (this.frame !== null) {
      cancelAnimationFrame(this.frame)
      this.frame = null
    }
  }
}
