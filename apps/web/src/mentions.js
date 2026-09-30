// A group message can address one member with `@name`. The picker offers only the
// current roster, so a name that is not a member is left as ordinary text rather
// than silently routing the request somewhere.

// The `@fragment` being typed at the caret, or null. A fragment only counts at a
// word boundary, so `mail@example.com` is never treated as a mention in progress.
export function mentionAt(text, caret = text.length) {
  const before = text.slice(0, caret)
  const at = before.lastIndexOf('@')
  if (at < 0) return null
  if (at > 0 && /[\p{L}\p{N}]/u.test(before[at - 1])) return null
  // The fragment ends at the first character that cannot be part of a name, so
  // punctuation typed after the name is never swallowed by the completion.
  const match = /^[^\s,.;:!?()[\]{}"']*/.exec(before.slice(at + 1))
  if (!match) return null
  // A space already after the name means the mention was finished, not typed.
  if (/^\s/.test(before.slice(at + 1 + match[0].length))) return null
  return {start: at, fragment: match[0]}
}

export function mentionPrefix(text, caret = text.length) {
  return mentionAt(text, caret)?.fragment ?? null
}

// Members matching the fragment, in roster order. Empty fragment offers everyone.
export function mentionCandidates(text, members, caret = text.length) {
  const fragment = mentionPrefix(text, caret)
  if (fragment === null) return []
  const wanted = fragment.toLowerCase()
  return [...new Set((members || []).filter(Boolean))]
    .filter(name => name.toLowerCase().startsWith(wanted))
    .slice(0, 8)
}

// Replace the fragment with a completed `@name `. Punctuation typed after the
// fragment is kept, so `@alice,` becomes `@alice,` and not `@alice ,`.
export function applyMention(text, name, caret = text.length) {
  const before = text.slice(0, caret)
  const fragment = mentionPrefix(text, caret)
  if (fragment === null) return text
  // Locate the `@` rather than deriving it from the fragment length: the caret can
  // sit past punctuation, so the fragment is not always `caret - start`.
  // Replace the name itself, not everything up to the caret: the caret can sit past
  // punctuation (`@alice,`), and that punctuation must survive the completion.
  const start = before.lastIndexOf('@')
  const end = start + 1 + fragment.length
  // The rest of the line already provides the separator, so add one only when the
  // name lands at the very end of the draft.
  const trailing = end >= text.length ? ' ' : ''
  return text.slice(0, start) + '@' + name + trailing + text.slice(end)
}

// Names already addressed in this draft, so the UI can show who will answer.
export function addressedNames(text) {
  return [...new Set((text.match(/@[\w./-]+/g) || []).map(m => m.slice(1)))]
}

// Which key the open picker consumes. While a list is showing, Enter and Tab
// accept an Agent instead of sending the message, which is what makes the picker
// usable without a mouse.
export function mentionKey(key, choices, composing = false) {
  if (!choices.length || composing) return 'pass'
  if (key === 'ArrowDown') return 'next'
  if (key === 'ArrowUp') return 'previous'
  if (key === 'Enter' || key === 'Tab') return 'accept'
  if (key === 'Escape') return 'dismiss'
  return 'pass'
}

// The highlighted row after moving by `delta`. It wraps, so the last row reaches
// the first with one more keypress, and it clamps on an empty list.
export function moveHighlight(current, delta, count) {
  if (count <= 0) return -1
  return (current + delta + count) % count
}

// The row to accept right now, guarding an index left over from a longer list.
export function highlighted(choices, index) {
  return index >= 0 && index < choices.length ? choices[index] : null
}
