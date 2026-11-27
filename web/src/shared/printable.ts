// A label comes from the node, and an older one may hold a character that reorders or hides how a line reads: a bidirectional
// override turns "Alice" into "ecilA" in a browser as it does on a terminal. The node refuses to write such a label now and
// prints each of them as `?` (`is_hidden_char` and `printable` in agency-proto); the screens say it the same way, here, once.
// The set: a control character (Unicode Cc), the bidirectional controls U+061C, U+200E, U+200F, U+202A to U+202E and U+2066 to
// U+2069, the zero-width space U+200B, the word joiner U+2060, and the byte-order mark U+FEFF. The joiners U+200C and U+200D
// (Persian, Indic scripts, emoji) and the emoji tag characters stay: none of them can reorder text.
const HIDDEN = /[\u0000-\u001F\u007F-\u009F\u061C\u200B\u200E\u200F\u202A-\u202E\u2060\u2066-\u2069\uFEFF]/g

/** The text with each hidden character as `?`. */
export const printable = (text: string): string => text.replace(HIDDEN, '?')

// Text that an agent or a person wrote (a title, a note, a summary) has line breaks and tabs, which are controls too and cannot reorder
// a line the way the rest of the set can: the same set, without U+0009, U+000A, and U+000D.
const HIDDEN_IN_TEXT = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F\u061C\u200B\u200E\u200F\u202A-\u202E\u2060\u2066-\u2069\uFEFF]/g

/** A text with each hidden character as `?`, its line breaks and tabs kept. */
export const printableText = (text: string): string => text.replace(HIDDEN_IN_TEXT, '?')

/**
 * What is one line in an answer, and printed as a label is: a `label`, the label of the option a decision chose, the `title` of a
 * request, of a work item, and of an event of the feed (a line the owner decides on, and a hidden character can reorder it as it can
 * a label), the `summary` of a message, and what a participant says it is doing (`status`).
 */
const LABEL_KEYS: ReadonlySet<string> = new Set(['label', 'choiceLabel', 'title', 'summary', 'status'])

/** What an agent or a person wrote with lines in an answer: the context and the document of a request, the detail and the text of an option, the text of a thread's message, a decision's text. */
const TEXT_KEYS: ReadonlySet<string> = new Set(['context', 'document', 'detail', 'text'])

/** What the node wrote in a string that was made printable, by its key, kept on the object only when making it printable changed it. */
const GIVEN_KEY = 'asGiven'

/**
 * The value the node gave for `key` of an object `labelsMadePrintable` returned: what is shown is made printable, but what is sent
 * back to the node (a decision's title and the label of its choice, for the log) is the value it gave, never the shown one.
 */
export function asGiven(object: object, key: string): string | undefined {
  const given = (object as { [GIVEN_KEY]?: Record<string, string> })[GIVEN_KEY]?.[key]
  const shown = (object as Record<string, unknown>)[key]
  return given ?? (typeof shown === 'string' ? shown : undefined)
}

/** The string as it is drawn, or as it was given when making it printable changed nothing (`undefined` then). */
const madePrintable = (key: string, text: string): string | undefined => {
  if (LABEL_KEYS.has(key)) return printable(text)
  if (TEXT_KEYS.has(key)) return printableText(text)
  return undefined
}

/**
 * An answer of the node with every label, and every text an agent or a person wrote, in it made printable, wherever it lies: the same answer, rebuilt, so that no screen draws a
 * label that was not. A snapshot's `relayedFor` is a map from a request to the label of whom it is for, so its values are labels.
 * An object that had something made printable also holds what the node gave, as `asGiven` (read through `asGiven()`), for what is sent back.
 */
export function labelsMadePrintable(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(labelsMadePrintable)
  if (typeof value !== 'object' || value === null) return value
  const given: Record<string, string> = {}
  const rebuilt = Object.fromEntries(
    Object.entries(value).map(([key, inside]) => {
      const drawn = typeof inside === 'string' ? madePrintable(key, inside) : undefined
      if (drawn !== undefined) {
        if (drawn !== inside) given[key] = inside as string
        return [key, drawn]
      }
      if (key === 'relayedFor' && typeof inside === 'object' && inside !== null && !Array.isArray(inside) && Object.values(inside).every((one) => typeof one === 'string')) {
        return [key, Object.fromEntries(Object.entries(inside).map(([request, label]) => [request, printable(String(label))]))]
      }
      return [key, labelsMadePrintable(inside)]
    }),
  )
  return Object.keys(given).length === 0 ? rebuilt : { ...rebuilt, [GIVEN_KEY]: given }
}
