// The history answer comes from the node and is read once, here, before a screen sees it: the node has worked out what each version was signed
// with and what it changed, so a screen draws that and decides nothing of its own. Every entry has a version and its words; a signature check
// is one of the three the screen can say; and each change is made printable (it is text from the org's own versions, which may hold a character
// that reorders a line), so that none is drawn as the node gave it.
import { printable } from '../shared/printable'
import type { HistoryEntry, OrgHistory, SignatureCheck } from './clientApi'
import { MalformedAnswer } from './evaluationAnswers'

type Raw = Record<string, unknown>

const asObject = (value: unknown): Raw => (typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Raw) : {})

const version = (value: unknown, what: string): number => {
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 1) throw new MalformedAnswer(what)
  return value
}

const optionalText = (value: unknown, what: string): string | undefined => {
  if (value === undefined) return undefined
  if (typeof value !== 'string' || value === '') throw new MalformedAnswer(what)
  return value
}

function entryOf(raw: unknown): HistoryEntry {
  const given = asObject(raw)
  if (typeof given.isKept !== 'boolean') throw new MalformedAnswer('kept flag')
  if (!Array.isArray(given.changes)) throw new MalformedAnswer('changes')
  const signature = given.signature
  if (signature !== undefined && signature !== 'valid' && signature !== 'invalid' && signature !== 'notChecked') throw new MalformedAnswer('signature')
  if (given.follows !== undefined && typeof given.follows !== 'boolean') throw new MalformedAnswer('follows flag')
  const at = optionalText(given.at, 'time')
  const signedBy = optionalText(given.signedBy, 'signer')
  return {
    version: version(given.version, 'version'),
    ...(at === undefined ? {} : { at }),
    ...(signature === undefined ? {} : { signature: signature as SignatureCheck }),
    ...(signedBy === undefined ? {} : { signedBy: printable(signedBy) }),
    ...(given.follows === undefined ? {} : { follows: given.follows }),
    ...(given.comparedWith === undefined ? {} : { comparedWith: version(given.comparedWith, 'compared version') }),
    changes: given.changes.map((change) => {
      if (typeof change !== 'string' || change === '') throw new MalformedAnswer('change')
      return printable(change)
    }),
    isKept: given.isKept,
  }
}

export function readHistory(raw: unknown): OrgHistory {
  const given = asObject(raw)
  if (!Array.isArray(given.entries)) throw new MalformedAnswer('entries')
  if (given.more !== undefined && typeof given.more !== 'boolean') throw new MalformedAnswer('more flag')
  return { entries: given.entries.map(entryOf), ...(given.more === true ? { more: true } : {}) }
}
