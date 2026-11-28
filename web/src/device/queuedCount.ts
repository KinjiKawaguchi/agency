import { MOST_DECISIONS } from '../api/queries'

/**
 * How many of the decision log's entries the node holds undelivered (`queued`), and whether that is all there are: the node gives at most
 * `MOST_DECISIONS` of the newest, so when the answer is full there may be more than were read, and the count is "at least" that many.
 */
export function queuedCount(entries: ReadonlyArray<{ state: { kind: string } }>): { count: number; isAtLeast: boolean } {
  return { count: entries.filter((entry) => entry.state.kind === 'queued').length, isAtLeast: entries.length >= MOST_DECISIONS }
}
