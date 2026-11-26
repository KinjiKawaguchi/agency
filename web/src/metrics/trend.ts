// The hour-by-hour trend of the node's own response time, worked out of the series' `byHour`. An hour has no buckets, so
// the p95 of different routes cannot be put together; what is drawn for a group is the slowest route's p95 of each hour,
// and the group's own p95 in that hour is at most that (when every part of a whole has 95 of 100 within a time, so has
// the whole). That is an upper bound, and it is written "at most" like every percentile.
import type { LatencySeries } from '../api/metricsApi'

const HOUR_MS = 3_600_000

/** The longest range whose hours the node gives (7 days). */
export const MAX_HOURS = 24 * 7

/** What an hour says: nothing (no call), the p95 as the upper bound of its bucket, or slower than the last bucket. */
export type HourValue = { kind: 'none' } | { kind: 'atMost'; ms: number } | { kind: 'beyond' }

export interface HourPoint {
  /** The start of the hour, in milliseconds since the epoch. */
  at: number
  /** The calls in the hour, over every route of the group. */
  count: number
  value: HourValue
}

/** The start of each hour of `from` to `until`, oldest first; none when the range is not hours of at most 7 days. */
export function hoursBetween(from: string, until: string): number[] {
  const start = Date.parse(from)
  const end = Date.parse(until)
  if (Number.isNaN(start) || Number.isNaN(end) || end <= start) return []
  const count = Math.round((end - start) / HOUR_MS)
  return count > MAX_HOURS ? [] : Array.from({ length: count }, (_, index) => start + index * HOUR_MS)
}

/** One point for each of the hours: the slowest p95 of the series' calls in it, a gap where none was made. */
export function trendOf(series: readonly LatencySeries[], hours: readonly number[]): HourPoint[] {
  const byHour = new Map<number, { count: number; slowest: number | 'beyond' | undefined }>()
  for (const one of series) {
    for (const entry of one.byHour) {
      if (entry.count <= 0) continue
      const at = Date.parse(entry.hour)
      const known = byHour.get(at) ?? { count: 0, slowest: undefined }
      const slowest = entry.p95Ms === undefined || known.slowest === 'beyond' ? 'beyond' : known.slowest === undefined ? entry.p95Ms : Math.max(known.slowest, entry.p95Ms)
      byHour.set(at, { count: known.count + entry.count, slowest })
    }
  }
  return hours.map((at): HourPoint => {
    const found = byHour.get(at)
    if (found === undefined || found.slowest === undefined) return { at, count: 0, value: { kind: 'none' } }
    return { at, count: found.count, value: found.slowest === 'beyond' ? { kind: 'beyond' } : { kind: 'atMost', ms: found.slowest } }
  })
}

/** Whether any hour has a call: a range of more than 7 days comes with totals only, and a range of hours with none has nothing to draw. */
export const hasHours = (points: readonly HourPoint[]): boolean => points.some((point) => point.value.kind !== 'none')
