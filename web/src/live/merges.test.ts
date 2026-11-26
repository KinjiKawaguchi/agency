import type { TaggedEvent } from './stream'
import { BIN_MS, binsOf, countsOf, lastHour, medianOf, startOfDay } from './merges'

const NOW = Date.parse('2026-11-26T10:00:00Z')
const merged = (minutesAgo: number): TaggedEvent => ({ org: 'acme', team: 'app', event: { id: `b-${minutesAgo}`, at: new Date(NOW - minutesAgo * 60_000).toISOString(), kind: 'prMerged', ref: 'app#1', title: 'x' } })
const opened = (minutesAgo: number): TaggedEvent => ({ org: 'acme', team: 'app', event: { id: `o-${minutesAgo}`, at: new Date(NOW - minutesAgo * 60_000).toISOString(), kind: 'prOpened', ref: 'app#1', title: 'x' } })

describe('merges per five minutes', () => {
  it('counts the merges of the last hour in twelve bins, the newest last, and nothing that is older, newer, or not a merge', () => {
    const bins = binsOf([merged(1), merged(2), merged(7), merged(59), merged(61), merged(-3), opened(1)], NOW)
    expect(bins).toHaveLength(12)
    expect(bins[11]).toBe(2)
    expect(bins[10]).toBe(1)
    expect(bins[0]).toBe(1)
    expect(bins.reduce((sum, count) => sum + count, 0)).toBe(4)
    expect(lastHour([merged(1), merged(61)], NOW)).toBe(1)
  })

  it('puts a merge exactly on a bin’s edge in the older bin it belongs to', () => {
    const bins = binsOf([{ ...merged(0), event: { ...merged(0).event, at: new Date(NOW - BIN_MS).toISOString() } }], NOW)
    expect(bins[10]).toBe(1)
  })
})

describe('medianOf', () => {
  it('is the middle value, the mean of the two middle ones rounded down, or nothing', () => {
    expect(medianOf([5, 1, 3])).toBe(3)
    expect(medianOf([1, 2])).toBe(1)
    expect(medianOf([])).toBeUndefined()
  })
})

describe('the page’s own counts', () => {
  it('never put the day below the hour, however the evaluation and the feed disagree', () => {
    // 18 merges in the last hour, and an evaluation that says 9 for the day.
    const events = Array.from({ length: 18 }, (_, index) => merged(index * 3 + 1))
    const counts = countsOf(events, NOW, 9)
    expect(counts.lastHour).toBe(18)
    expect(counts.today).toBeGreaterThanOrEqual(counts.lastHour)
  })

  it('holds for every moment of the day after the first hour, and for an evaluation that is behind or missing', () => {
    const events = Array.from({ length: 40 }, (_, index) => merged(index * 7 + 1))
    const midnight = startOfDay(NOW)
    for (let minute = 61; minute < 24 * 60; minute += 37) {
      for (const evaluationToday of [undefined, 0, 3, 100]) {
        const at = midnight + minute * 60_000
        const counts = countsOf(events.map((one) => ({ ...one, event: { ...one.event, at: new Date(at - (NOW - Date.parse(one.event.at))).toISOString() } })), at, evaluationToday)
        expect(counts.today).toBeGreaterThanOrEqual(counts.lastHour)
      }
    }
  })

  it('takes the evaluation’s count for the day when it is more than the feed has seen, and counts nothing the feed has not yet sent', () => {
    expect(countsOf([merged(1)], NOW, 9)).toEqual({ lastHour: 1, today: 9 })
    expect(countsOf([merged(-5)], NOW, undefined)).toEqual({ lastHour: 0, today: 0 })
  })
})
