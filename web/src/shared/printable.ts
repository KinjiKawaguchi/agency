// A label comes from the node, and an older one may hold a character that reorders or hides how a line reads: a bidirectional
// override turns "Alice" into "ecilA" in a browser as it does on a terminal. The node refuses to write such a label now and
// prints each of them as `?` (`is_hidden_char` and `printable` in agency-proto); the screens say it the same way, here, once.
// The set: a control character (Unicode Cc), the bidirectional controls U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to
// U+2069, the zero-width space U+200B, the word joiner U+2060, and the byte-order mark U+FEFF. The joiners U+200C and U+200D
// (Persian, Indic scripts, emoji) and the emoji tag characters stay: none of them can reorder text.
const HIDDEN = /[\u0000-\u001F\u007F-\u009F\u061C\u200B\u200E\u200F\u202A-\u202E\u2060\u2066-\u2069\uFEFF]/g

/** The text with each hidden character as `?`. */
export const printable = (text: string): string => text.replace(HIDDEN, '?')

/** What the node calls a label in an answer: a `label`, and the label of the option a decision chose. */
const LABEL_KEYS: ReadonlySet<string> = new Set(['label', 'choiceLabel'])

/**
 * An answer of the node with every label in it made printable, wherever it lies: the same answer, rebuilt, so that no screen draws a
 * label that was not. A snapshot's `relayedFor` is a map from a request to the label of whom it is for, so its values are labels.
 */
export function labelsMadePrintable(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(labelsMadePrintable)
  if (typeof value !== 'object' || value === null) return value
  return Object.fromEntries(
    Object.entries(value).map(([key, inside]) => {
      if (LABEL_KEYS.has(key) && typeof inside === 'string') return [key, printable(inside)]
      if (key === 'relayedFor' && typeof inside === 'object' && inside !== null && !Array.isArray(inside) && Object.values(inside).every((one) => typeof one === 'string')) {
        return [key, Object.fromEntries(Object.entries(inside).map(([request, label]) => [request, printable(String(label))]))]
      }
      return [key, labelsMadePrintable(inside)]
    }),
  )
}
