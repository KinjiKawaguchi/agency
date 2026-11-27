import type { Envelope, Signed } from '../api/clientApi'
import { afterFailure, backoffMs, classify, dueEntries, nextDueAt, sendAgain, type OutboxEntry } from './entry'

const envelope = { body: { id: 'e-1' }, signer: 'alice', signature: 'sig' } as unknown as Signed<Envelope>
const entry = (overrides: Partial<OutboxEntry> = {}): OutboxEntry => ({
  id: 'e-1', org: 'acme', team: 'app', itemKey: 'acme:app:r-1', itemTitle: 'Ship it?', text: 'Go.', draft: { text: 'Go.', choice: null }, envelope, createdAt: '2026-11-27T01:00:00Z', attempts: 0, phase: 'sending', ...overrides,
})
const NOW = Date.parse('2026-11-27T01:00:00Z')

describe('the wait before the next try', () => {
  it('is 5 seconds, 15 seconds, a minute, 5 minutes, and 5 minutes after that', () => {
    expect([1, 2, 3, 4, 5, 6, 40].map(backoffMs)).toEqual([5_000, 15_000, 60_000, 300_000, 300_000, 300_000, 300_000])
    expect(backoffMs(0)).toBe(5_000)
  })
})

describe('how a try that did not take ends', () => {
  it('is tried again for no answer, a busy node, a 408, a 425 and a 429', () => {
    expect(classify({ message: 'Failed to fetch', isNetwork: true })).toEqual({ kind: 'retry', reason: 'Failed to fetch' })
    for (const status of [500, 502, 503, 504, 408, 425, 429]) expect(classify({ status, message: 'busy' }).kind, String(status)).toBe('retry')
  })

  it('is stopped, with the node’s own words, for a refusal that will not change: 400, 401, 403, 404, 409 and the rest of the 4xx', () => {
    for (const status of [400, 401, 403, 404, 409, 410, 422]) expect(classify({ status, message: 'the team’s node refused: no such request' }), String(status)).toEqual({ kind: 'stop', reason: 'the team’s node refused: no such request' })
  })

  it('waits for the sign-in when it has ended, and says nothing of why', () => {
    expect(classify({ message: 'x', isSignIn: true })).toEqual({ kind: 'sign-in' })
  })

  it('takes a Retry-After of a 503, in seconds, up to an hour', () => {
    expect(classify({ status: 503, message: 'full', retryAfterSeconds: 90 })).toEqual({ kind: 'retry', reason: 'full', retryAfterMs: 90_000 })
    expect(classify({ status: 503, message: 'full', retryAfterSeconds: 999_999 })).toMatchObject({ retryAfterMs: 3_600_000 })
  })
})

describe('a kept decision after a try', () => {
  it('waits for the next try: one more attempt, the time, and why; the envelope is the same', () => {
    const after = afterFailure(entry(), { kind: 'retry', reason: 'Failed to fetch' }, NOW)
    expect(after).toMatchObject({ attempts: 1, phase: 'retrying', reason: 'Failed to fetch', nextAt: '2026-11-27T01:00:05.000Z' })
    expect(after.envelope).toBe(envelope)
    expect(afterFailure(after, { kind: 'retry', reason: 'again' }, NOW + 5_000)).toMatchObject({ attempts: 2, nextAt: '2026-11-27T01:00:20.000Z' })
  })

  it('waits at least as long as the node asked, and never less than its own backoff', () => {
    expect(afterFailure(entry(), { kind: 'retry', reason: 'full', retryAfterMs: 60_000 }, NOW).nextAt).toBe('2026-11-27T01:01:00.000Z')
    expect(afterFailure(entry(), { kind: 'retry', reason: 'full', retryAfterMs: 1_000 }, NOW).nextAt).toBe('2026-11-27T01:00:05.000Z')
  })

  it('stops for a refusal, keeps the reason, and has no next time', () => {
    const after = afterFailure(entry({ phase: 'retrying', nextAt: '2026-11-27T01:00:05.000Z', attempts: 2 }), { kind: 'stop', reason: 'refused' }, NOW)
    expect(after).toMatchObject({ phase: 'failed', reason: 'refused', attempts: 3 })
    expect(after).not.toHaveProperty('nextAt')
  })

  it('stays as it was while the sign-in is gone: no attempt is counted', () => {
    expect(afterFailure(entry({ attempts: 1 }), { kind: 'sign-in' }, NOW)).toMatchObject({ attempts: 1, phase: 'sending' })
  })

  it('is sent again at once by the owner, whatever it waited for, with the reason of the last try kept until the next', () => {
    const again = sendAgain(entry({ phase: 'failed', reason: 'refused', attempts: 3 }))
    expect(again).toMatchObject({ phase: 'sending', reason: 'refused', attempts: 3 })
    expect(sendAgain(entry({ phase: 'retrying', nextAt: '2026-11-27T05:00:00Z' }))).not.toHaveProperty('nextAt')
  })

  it('does not change the entry it is given', () => {
    const before = entry()
    afterFailure(before, { kind: 'stop', reason: 'x' }, NOW)
    sendAgain(before)
    expect(before).toEqual(entry())
  })
})

describe('which kept decisions are due', () => {
  const entries = [entry({ id: 'a' }), entry({ id: 'b', phase: 'retrying', nextAt: '2026-11-27T01:00:05.000Z' }), entry({ id: 'c', phase: 'failed' }), entry({ id: 'd', phase: 'retrying', nextAt: '2026-11-27T01:00:00.000Z' })]

  it('are those on their way and not in flight, and those whose time has come; never one that failed', () => {
    expect(dueEntries(entries, new Set(), NOW).map((one) => one.id)).toEqual(['a', 'd'])
    expect(dueEntries(entries, new Set(), NOW + 5_000).map((one) => one.id)).toEqual(['a', 'b', 'd'])
    expect(dueEntries(entries, new Set(['a']), NOW).map((one) => one.id)).toEqual(['d'])
  })

  it('is, for the timer, the earliest time a waiting one is due; nothing when none waits or the time is not one', () => {
    expect(nextDueAt(entries)).toBe(Date.parse('2026-11-27T01:00:00.000Z'))
    expect(nextDueAt([entry(), entry({ phase: 'failed' })])).toBeUndefined()
    expect(nextDueAt([entry({ phase: 'retrying', nextAt: 'soon' })])).toBeUndefined()
  })
})
