// A decision the owner made and the node has not yet taken: the signed envelope, kept until the node says it has it. It is made once,
// when the owner decides, and sent as many times as it takes: always the same envelope with the same id, which the node keeps once.
import type { Envelope, Signed } from '../api/clientApi'

/** Where a kept decision stands: on its way, waiting for the next try, or stopped for a reason that will not change by itself. */
export type Phase = 'sending' | 'retrying' | 'failed'

/** What was typed, kept so that discarding the decision gives it back to the desk. */
export interface DecisionDraft {
  text: string
  choice: string | null
}

export interface OutboxEntry {
  /** The envelope's id: the same for every try, and what the node knows the decision by. */
  id: string
  org: string
  team: string
  /** The request it answers, as the desk keys it (`org:team:request`), so that the request stays off the pile. */
  itemKey: string
  /** The request's title as the owner saw it, for the tray. */
  itemTitle: string
  /** What was decided, for the tray: the label of the option chosen and the words, as the owner gave them. */
  chosenLabel?: string
  text: string
  draft: DecisionDraft
  envelope: Signed<Envelope>
  /** The notes the node keeps in its log with the envelope. */
  title?: string
  choiceLabel?: string
  createdAt: string
  /** How many tries have ended without the node taking it. */
  attempts: number
  phase: Phase
  /** The next try, as a time, while `retrying`. */
  nextAt?: string
  /** Why the last try did not take, as the node (or the browser) said it. */
  reason?: string
}

/** The wait before try number `attempts` + 1, in milliseconds: 5 seconds, 15 seconds, a minute, 5 minutes, and 5 minutes after that. */
export const BACKOFF_MS: readonly number[] = [5_000, 15_000, 60_000, 300_000]

export const backoffMs = (attempts: number): number => BACKOFF_MS[Math.min(Math.max(attempts, 1), BACKOFF_MS.length) - 1] ?? 300_000

/** How a try that did not take ends: it is tried again, it stops for good, or all sending waits for the sign-in. */
export type Failure = { kind: 'retry'; reason: string; retryAfterMs?: number } | { kind: 'stop'; reason: string } | { kind: 'sign-in' }

/** HTTP statuses that are tried again: the node or the way to it was busy, not a refusal. */
const RETRIED_STATUSES: readonly number[] = [408, 425, 429]

export function classify(failure: { status?: number; message: string; isNetwork?: boolean; isSignIn?: boolean; retryAfterSeconds?: number }): Failure {
  if (failure.isSignIn === true) return { kind: 'sign-in' }
  if (failure.isNetwork === true) return { kind: 'retry', reason: failure.message }
  const status = failure.status
  if (status !== undefined && (status >= 500 || RETRIED_STATUSES.includes(status))) {
    const retryAfterMs = failure.retryAfterSeconds === undefined ? undefined : Math.min(failure.retryAfterSeconds, 3600) * 1000
    return { kind: 'retry', reason: failure.message, ...(retryAfterMs === undefined ? {} : { retryAfterMs }) }
  }
  return { kind: 'stop', reason: failure.message }
}

/** The entry after a try that ended: waiting for the next, or stopped. A refusal that will not change gives `failed`; "Send again" starts it over. */
export function afterFailure(entry: OutboxEntry, failure: Failure, now: number): OutboxEntry {
  if (failure.kind === 'sign-in') return { ...entry, phase: 'sending' }
  const attempts = entry.attempts + 1
  if (failure.kind === 'stop') return { ...withoutNext(entry), attempts, phase: 'failed', reason: failure.reason }
  const wait = failure.retryAfterMs === undefined ? backoffMs(attempts) : Math.max(failure.retryAfterMs, backoffMs(attempts))
  return { ...entry, attempts, phase: 'retrying', nextAt: new Date(now + wait).toISOString(), reason: failure.reason }
}

const withoutNext = ({ nextAt: _nextAt, ...rest }: OutboxEntry): OutboxEntry => rest

/** "Send again": the same envelope, at once, whatever the wait was. */
export const sendAgain = (entry: OutboxEntry): OutboxEntry => ({ ...withoutNext(entry), phase: 'sending' })

/** A waiting entry is due at its time, and at once when its time is missing or is not one: it is never left waiting for nothing. */
const isDue = (entry: OutboxEntry, now: number): boolean => {
  const at = entry.nextAt === undefined ? Number.NaN : Date.parse(entry.nextAt)
  return Number.isNaN(at) || at <= now
}

/** The entries that should be tried now: on their way and not in flight, or waiting and their time has come. */
export function dueEntries(entries: readonly OutboxEntry[], inFlight: ReadonlySet<string>, now: number): OutboxEntry[] {
  return entries.filter((entry) => !inFlight.has(entry.id) && (entry.phase === 'sending' || (entry.phase === 'retrying' && isDue(entry, now))))
}

/** When the earliest waiting entry is due, for the timer; nothing when none waits. */
export function nextDueAt(entries: readonly OutboxEntry[]): number | undefined {
  const times = entries.flatMap((entry) => (entry.phase === 'retrying' && entry.nextAt !== undefined && !Number.isNaN(Date.parse(entry.nextAt)) ? [Date.parse(entry.nextAt)] : []))
  return times.length === 0 ? undefined : Math.min(...times)
}
