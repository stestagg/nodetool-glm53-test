// The packaging gestures' client-side decisions, pure: data in, decision
// out. The server derives the packaging itself from the definition it
// holds — the browser assembles nothing of the groups format — so what
// lives here is the little the gesture needs before it can ask: the name
// suggestion the dialog opens with, and which of the selected nodes are
// group instances for unpack to act on.

// The first available name in the Group, Group 2, ... sequence — Enter
// alone packages, because the suggestion is one the server will take.
export function groupSuggestion(groups) {
  const taken = new Set((groups ?? []).map((group) => group.name))
  for (let n = 1; ; n += 1) {
    const name = n === 1 ? 'Group' : `Group ${n}`
    if (!taken.has(name)) return name
  }
}

// The group instances among `selected`: the nodes whose type reference a
// document group defines — unpack's operation runs once per one of them,
// the ordinary nodes in the selection left as they are.
export function groupInstances(selected, groups) {
  const names = new Set((groups ?? []).map((group) => group.name))
  return selected.filter((node) => names.has(node.type_ref)).map((node) => node.uuid)
}
