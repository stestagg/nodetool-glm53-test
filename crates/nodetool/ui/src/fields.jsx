// The scalar fields: the one both views of a single input's value share —
// the node's inline field and the sidebar's parameter row render and
// commit the same scalar text, so an edit in either view appears in the
// other — and, mixed, the multi-selection's common field on the same
// commit cycle. Which inputs get a field is read off the listing's
// base-scalar fact — an input whose declared types include a core base
// scalar, whatever union it declares; an input declared only on plugin
// custom types is opaque to the editor and gets none.
//
// A declared choice — a setting whose value is one of a fixed set the
// listing carries — renders as the select below instead, in the same two
// views and through the same commit seam.

import { createContext, useContext, useEffect, useState } from 'react'

// The editing seam the field commits through: the editor's request
// function, provided once at the shell. Null until the connection is
// live.
export const EditContext = createContext(null)

export function useEdit() {
  return useContext(EditContext)
}

// The editing lock a run holds: while a run is on, the definition is held
// still, and every field is inert — a value's views show what is stored
// and take no edit. The shell provides it once; the fields read it.
export const LockContext = createContext(false)

export function useLocked() {
  return useContext(LockContext)
}

// The keyboard wire in progress: the port it runs from — node, port, and
// the handle's side — or null. The shell holds it and lands or stands it
// down; the ports read it to mark their own origin handle. Outside the
// editor — a component rendered on its own — no wire runs and none starts.
export const WireContext = createContext({ wire: null, setWire: () => {} })

// The rule for which inputs get an editable field: a declared type that
// is a core base scalar makes the port scalar-possible. A type reference
// the listing does not classify reads as not-a-base-scalar; so does a
// listing not yet arrived.
export function scalarPossible(port, baseScalars = {}) {
  return port.type_refs.some((ref) => baseScalars[ref] === true)
}

// A stored scalar as its text, for a field to show: the boolean,
// integer, or float as written, the string bare, no value as empty — the
// same empty that commits as unset. A JSON null in the definition — what
// a non-finite float becomes, which the envelope cannot carry — reads as
// empty too: the field never shows a value the definition does not hold.
// (An integer past 2^53 rounds through the JSON number the same way: the
// envelope's precision, not the reading's.)
export function scalarText(value) {
  if (value === undefined || value === null) return ''
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  return String(value)
}

// Commit an input's value through the editing seam. Empty text unsets —
// the definition holds no empty-string stand-in — anything else crosses
// as the raw text: the server reads it by the file format's own reading,
// so what is stored is what a hand-written file would carry.
export function commitParameter(edit, uuid, input, text) {
  if (text === '') edit('set_parameter', { uuid, input })
  else edit('set_parameter', { uuid, input, value: text })
}

// The commit-cycling field: Enter or leaving the field commits, Escape
// discards an uncommitted draft, and an unchanged commit sends nothing.
// The draft is local; the stored value arrives by definition push and
// re-seeds the field whenever it changes, so neither view of the stored
// value is authoritative in the browser. A run's lock disables the
// field. `mixed` bends the cycle to the multi-selection's commit: the
// marker rides the placeholder — blank alone must mean unset-everywhere
// — Enter on an untouched mixed field stays the deliberate clear, empty
// meaning unset on every node, while leaving the field commits only an
// edit, so a stray click away from a mixed field cannot unset anything.
export function Field({ value, mixed = false, placeholder = '', onCommit, className = '', ...rest }) {
  const stored = scalarText(value)
  const [draft, setDraft] = useState(stored)
  const [edited, setEdited] = useState(false)
  useEffect(() => {
    setDraft(stored)
    setEdited(false)
  }, [stored, mixed])
  const locked = useLocked()
  const commit = () => {
    if (draft !== stored || mixed) onCommit(draft)
  }
  return (
    <input
      className={`field nodrag ${className}`}
      value={draft}
      placeholder={mixed ? 'mixed' : placeholder}
      disabled={locked}
      onChange={(event) => {
        setDraft(event.target.value)
        setEdited(true)
      }}
      onKeyDown={(event) => {
        if (event.key === 'Enter') commit()
        else if (event.key === 'Escape') {
          setDraft(stored)
          setEdited(false)
        }
      }}
      onBlur={() => {
        if (edited) commit()
      }}
      {...rest}
    />
  )
}

// The choice select: the field a declared setting gets, offering exactly
// the options the type declared. There is no half-typed draft to keep, so
// no commit cycle either — a pick commits at once — and the select shows
// what is stored rather than what is declared: a stored value outside the
// options, an unset choice included, sits as its own row until one of them
// replaces it, so the control never claims a value the definition does not
// hold. A run's lock disables it with every other field.
export function Select({ value, options, onCommit, className = '', ...rest }) {
  const stored = scalarText(value)
  const locked = useLocked()
  return (
    <select
      className={`field nodrag ${className}`}
      value={stored}
      disabled={locked}
      onChange={(event) => onCommit(event.target.value)}
      {...rest}
    >
      {!options.includes(stored) && (
        <option value={stored}>{stored === '' ? '—' : stored}</option>
      )}
      {options.map((option) => (
        <option key={option} value={option}>
          {option}
        </option>
      ))}
    </select>
  )
}
