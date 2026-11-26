import type { TaggedEvent } from './stream'
import { BIN_MS, binsOf, lastHour, medianOf } from './merges'

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
