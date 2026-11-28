// The mock node's `GET /api/orgs/{org}/history`: a history of a given length, paged as the node pages it. The node's own sample
// (`testdata/history.sample.json`) is what a short one looks like; a long one is made of its entries repeated under version numbers that go on, so that
// a screen is drawn against several pages. `after` keeps only the versions above it, and `limit` (1 to 1000, default 200) cuts the answer, with `more`
// when entries after the last one were left out. A bad `limit` or `after` is 400, as it is on the node.
import sample from '../../../testdata/history.sample.json' with { type: 'json' }
import type { HistoryEntry, OrgHistory } from '../api/clientApi.ts'

const DEFAULT_LIMIT = 200
const MOST_LIMIT = 1000

/** The org's history of `length` versions: the sample's, then more like its kept versions, oldest first. */
export function historyOf(length: number): readonly HistoryEntry[] {
  const known = sample.entries as readonly HistoryEntry[]
  return Array.from({ length }, (_none, index): HistoryEntry => {
    const version = index + 1
    const shown = known[index]
    if (shown !== undefined) return shown
    const day = String(((index + 3) % 28) + 1).padStart(2, '0')
    return { version, at: `2026-11-${day}T09:30:00Z`, signature: 'valid', signedBy: 'root key 3f9c-a1b2-77d0-e4c5', follows: true, changes: [`add member m${version} (Member ${version})`], isKept: true }
  })
}

const wholeNumber = (value: string | null, least: number, most: number): number | undefined | 'bad' => {
  if (value === null) return undefined
  const parsed = Number(value)
  return Number.isInteger(parsed) && parsed >= least && parsed <= most ? parsed : 'bad'
}

/** The page of `entries` that `url`'s query asks for, or the reason it is refused. */
export function pageOf(entries: readonly HistoryEntry[], url: URL): OrgHistory | { refused: string } {
  const limit = wholeNumber(url.searchParams.get('limit'), 1, MOST_LIMIT)
  const after = wholeNumber(url.searchParams.get('after'), 0, Number.MAX_SAFE_INTEGER)
  if (limit === 'bad') return { refused: 'limit must be a number from 1 to 1000.' }
  if (after === 'bad') return { refused: 'after must be a version number.' }
  const later = entries.filter((entry) => entry.version > (after ?? 0))
  const page = later.slice(0, limit ?? DEFAULT_LIMIT)
  return { entries: [...page], ...(page.length < later.length ? { more: true } : {}) }
}
