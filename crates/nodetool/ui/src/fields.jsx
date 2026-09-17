// The scalar field both views of an input's value share: the node's
// inline field and the sidebar's parameter row render and commit the same
// scalar text, so an edit in either view appears in the other. Which
// inputs get a field is read off the listing's base-scalar fact — an
// input whose declared types include a core base scalar, whatever union
// it declares; an input declared only on plugin custom types is opaque
// to the editor and gets none.

import { createContext, useContext, useEffect, useState } from 'react'

// The editing seam the field commits through: the editor's request
// function, provided once at the shell. Null until the connection is
// live.
export const EditContext = createContext(null)

export function useEdit() {
  return useContext(EditContext)
}

// The rule for which inputs get an editable field: a declared type that
// is a core base scalar makes the port scalar-possible. A type reference
// the listing does not classify reads as not-a-base-scalar; so does a
// listing not yet arrived.
export function scalarPossible(port, baseScalars = {}) {
  return port.type_refs.some((ref) => baseScalars[ref] === true)
}

// A stored scalar as its text, for a field to show: the boolean,
// integer, or float as written, the string bare, no value as empty — the
// same empty that commits as unset.
export function scalarText(value) {
  if (value === undefined) return ''
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  return String(value)
}

const INTEGER = /^-?\d+$/
const FLOAT = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/

// The text a field commits as the plain scalar the file format reads it
// as: boolean, integer, float, otherwise string. Nothing is validated
// beyond that — whether the value suits its port is compile time's
// business.
export function parseScalarText(text) {
  if (text === 'true') return true
  if (text === 'false') return false
  if (INTEGER.test(text)) return Number(text)
  if (FLOAT.test(text)) return Number(text)
  return text
}

// Commit an input's value through the editing seam. Empty text unsets —
// the definition holds no empty-string stand-in — anything else commits
// as the plain scalar its text reads as.
export function commitParameter(edit, uuid, input, text) {
  if (text === '') edit('set_parameter', { uuid, input })
  else edit('set_parameter', { uuid, input, value: parseScalarText(text) })
}

// The commit-cycling field: Enter or leaving the field commits, Escape
// discards an uncommitted draft, and an unchanged commit sends nothing.
// The draft is local; the stored value arrives by definition push and
// re-seeds the field whenever it changes, so neither view of the stored
// value is authoritative in the browser.
export function Field({ value, placeholder = '', onCommit, className = '', ...rest }) {
  const stored = scalarText(value)
  const [draft, setDraft] = useState(stored)
  useEffect(() => setDraft(stored), [stored])
  const commit = () => {
    if (draft !== stored) onCommit(draft)
  }
  return (
    <input
      className={`field nodrag ${className}`}
      value={draft}
      placeholder={placeholder}
      onChange={(event) => setDraft(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === 'Enter') commit()
        else if (event.key === 'Escape') setDraft(stored)
      }}
      onBlur={commit}
      {...rest}
    />
  )
}
