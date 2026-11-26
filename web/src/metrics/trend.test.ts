import type { LatencySeries } from '../api/metricsApi'
import { hasHours, hoursBetween, MAX_HOURS, trendOf } from './trend'

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
const hour = (index: number, count: number, p95Ms?: number) => ({ hour: new Date(START + index * HOUR).toISOString(), count, errors: 0, ...(p95Ms === undefined ? {} : { p95Ms }) })

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
    expect(points[0]).toEqual({ at: START, count: 13, value: { kind: 'atMost', ms: 250 } })
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
})
