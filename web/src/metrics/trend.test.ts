import type { LatencySeries } from '../api/metricsApi'
import { hasHours, hoursBetween, isExactTrend, MAX_HOURS, trendOf } from './trend'

const HOUR = 3_600_000
const START = Date.parse('2026-11-24T10:00:00Z')

const series = (byHour: LatencySeries['byHour']): LatencySeries => ({
  kind: 'serve',
  name: 'GET /api/me',
  status: '2xx',
  group: 'desk',
  isInNodeRead: true,
  count: byHour.reduce((sum, one) => sum + one.count, 0),
  errors: 0,
  sumMs: 0,
  buckets: Array.from({ length: 15 }, () => 0),
  byHour,
})
const hour = (index: number, count: number, p95Ms?: number, buckets?: number[]) => ({ hour: new Date(START + index * HOUR).toISOString(), count, errors: 0, ...(p95Ms === undefined ? {} : { p95Ms }), ...(buckets === undefined ? {} : { buckets }) })
const at = (counts: Record<number, number>): number[] => Array.from({ length: 15 }, (_, index) => counts[index] ?? 0)

describe('the hours of a range', () => {
  it('are each hour from the start to the end, oldest first', () => {
    const hours = hoursBetween('2026-11-24T10:00:00Z', '2026-11-24T13:00:00Z')
    expect(hours).toEqual([START, START + HOUR, START + 2 * HOUR])
  })

  it('are none for a range of more than 7 days, for an end that is not after the start, or for a time that is not one', () => {
    expect(hoursBetween('2026-11-24T10:00:00Z', new Date(START + (MAX_HOURS + 1) * HOUR).toISOString())).toEqual([])
    expect(hoursBetween('2026-11-24T10:00:00Z', new Date(START + MAX_HOURS * HOUR).toISOString())).toHaveLength(MAX_HOURS)
    expect(hoursBetween('2026-11-24T10:00:00Z', '2026-11-24T10:00:00Z')).toEqual([])
    expect(hoursBetween('2026-11-24T13:00:00Z', '2026-11-24T10:00:00Z')).toEqual([])
    expect(hoursBetween('soon', '2026-11-24T10:00:00Z')).toEqual([])
  })
})

describe('the trend of a group, hour by hour', () => {
  const hours = [START, START + HOUR, START + 2 * HOUR, START + 3 * HOUR]

  it('is a gap for an hour with no call, never a 0', () => {
    const points = trendOf([series([hour(0, 5, 25), hour(2, 3, 100)])], hours)
    expect(points.map((point) => point.value)).toEqual([{ kind: 'atMost', ms: 25 }, { kind: 'none' }, { kind: 'atMost', ms: 100 }, { kind: 'none' }])
    expect(points.map((point) => point.count)).toEqual([5, 0, 3, 0])
  })

  it('is an hour whose entry has no call as no call at all', () => {
    expect(trendOf([series([hour(0, 0, 25)])], hours)[0]?.value).toEqual({ kind: 'none' })
  })

  it('takes the slowest route’s p95 of the hour, which the group’s own p95 is at most, and adds the calls', () => {
    const points = trendOf([series([hour(0, 5, 25)]), series([hour(0, 7, 250)]), series([hour(0, 1, 50)])], hours)
    expect(points[0]).toEqual({ at: START, count: 13, value: { kind: 'atMost', ms: 250 }, isExact: false })
  })

  it('is beyond the last bucket when any route’s p95 of the hour is, whatever order they come in', () => {
    expect(trendOf([series([hour(0, 2)]), series([hour(0, 7, 250)])], hours)[0]?.value).toEqual({ kind: 'beyond' })
    expect(trendOf([series([hour(0, 7, 250)]), series([hour(0, 2)])], hours)[0]?.value).toEqual({ kind: 'beyond' })
  })

  it('finds an hour by the time it is, not by how it is written', () => {
    const offset = { hour: '2026-11-24T19:00:00+09:00', count: 4, errors: 0, p95Ms: 10 }
    expect(trendOf([series([offset])], hours)[0]?.value).toEqual({ kind: 'atMost', ms: 10 })
  })

  it('leaves out an hour outside the range, and has no hours to draw when no hour has a call', () => {
    expect(hasHours(trendOf([series([hour(9, 3, 10)])], hours))).toBe(false)
    expect(hasHours(trendOf([series([hour(1, 3, 10)])], hours))).toBe(true)
  })

  describe('with the buckets of each hour', () => {
    it('is the group’s own p95, read from the buckets added together, not the slowest route’s', () => {
      // 99 calls within 5 ms and 1 call within 1 s: the slowest route's p95 is 1 s, the group's own is 5 ms.
      const quick = series([hour(0, 99, 5, at({ 2: 99 }))])
      const slow = series([hour(0, 1, 1000, at({ 9: 1 }))])
      const [point] = trendOf([quick, slow], hours)
      expect(point).toEqual({ at: START, count: 100, value: { kind: 'atMost', ms: 5 }, isExact: true })
      expect(trendOf([quick, series([hour(0, 1, 1000)])], hours)[0]).toMatchObject({ value: { kind: 'atMost', ms: 1000 }, isExact: false })
    })

    it('takes the slowest route’s p95 for an hour when one route that had calls gave no buckets, in either order', () => {
      const given = series([hour(0, 99, 5, at({ 2: 99 }))])
      const missing = series([hour(0, 1, 1000)])
      expect(trendOf([given, missing], hours)[0]?.isExact).toBe(false)
      expect(trendOf([missing, given], hours)[0]?.isExact).toBe(false)
    })

    it('is exact where the hour has buckets and approximate only where it has none', () => {
      const points = trendOf([series([hour(0, 10, 25, at({ 4: 10 })), hour(1, 10, 100)])], hours)
      expect(points.map((point) => point.isExact)).toEqual([true, false, true, true])
      expect(isExactTrend(points)).toBe(false)
      expect(isExactTrend(trendOf([series([hour(0, 10, 25, at({ 4: 10 }))])], hours))).toBe(true)
    })

    it('is beyond the last bucket when the p95 of the added buckets is, and nothing for an hour with no call', () => {
      const points = trendOf([series([hour(0, 10, undefined, at({ 14: 4, 5: 6 }))])], hours)
      expect(points[0]?.value).toEqual({ kind: 'beyond' })
      expect(points[1]).toEqual({ at: START + HOUR, count: 0, value: { kind: 'none' }, isExact: true })
    })

    it('reads the 95th percentile of the buckets, not a higher one', () => {
      // 100 calls: 96 within 5 ms and 4 within 1 s. The 95th is the 95th call (5 ms); the 99th would be 1 s.
      expect(trendOf([series([hour(0, 100, 5, at({ 2: 96, 9: 4 }))])], hours)[0]?.value).toEqual({ kind: 'atMost', ms: 5 })
    })

    it('adds the buckets of routes of an hour and does not average their percentiles', () => {
      const a = series([hour(2, 50, 10, at({ 3: 50 }))])
      const b = series([hour(2, 50, 250, at({ 7: 50 }))])
      // 100 calls, half within 10 ms and half within 250 ms: the 95th is within 250 ms, not between the two.
      expect(trendOf([a, b], hours)[2]?.value).toEqual({ kind: 'atMost', ms: 250 })
    })
  })
})
