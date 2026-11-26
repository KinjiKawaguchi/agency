// The hour-by-hour trend of the node's own response time, worked out of the series' `byHour`. With the buckets of each hour
// (when the page asks for them) the group's own p95 is read from them added together, as the period's is. Without them
// the p95 of different routes cannot be put together, and what is drawn is the slowest route's p95 of each hour, which the
// group's own p95 is at most. Either way it is an upper bound of a bucket, and written "at most" like every percentile.
import type { LatencySeries } from '../api/metricsApi'
import { percentileOf } from './latency'

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
  /** Whether the value is the group's own p95, read from the hour's buckets added together; otherwise it is the slowest route's, which the group's is at most. */
  isExact: boolean
}

/** The start of each hour of `from` to `until`, oldest first; none when the range is not hours of at most 7 days. */
export function hoursBetween(from: string, until: string): number[] {
  const start = Date.parse(from)
  const end = Date.parse(until)
  if (Number.isNaN(start) || Number.isNaN(end) || end <= start) return []
  const count = Math.round((end - start) / HOUR_MS)
  return count > MAX_HOURS ? [] : Array.from({ length: count }, (_, index) => start + index * HOUR_MS)
}

interface HourSums {
  count: number
  slowest: number | 'beyond' | undefined
  /** The hour's buckets added up, while every series that had calls in it gave its own. */
  buckets: number[] | undefined
}

/** What an hour says of a group: the p95 of the hour's buckets added together when every route gave them, else the slowest route's p95. */
function pointOf(at: number, found: HourSums | undefined, useBuckets: boolean): HourPoint {
  if (found === undefined || found.count <= 0) return { at, count: 0, value: { kind: 'none' }, isExact: true }
  const exact = !useBuckets || found.buckets === undefined ? undefined : percentileOf(found.buckets, 95)
  if (exact !== undefined) return { at, count: found.count, value: exact, isExact: true }
  if (found.slowest === undefined) return { at, count: 0, value: { kind: 'none' }, isExact: true }
  return { at, count: found.count, value: found.slowest === 'beyond' ? { kind: 'beyond' } : { kind: 'atMost', ms: found.slowest }, isExact: false }
}

/**
 * One point for each of the hours, a gap where no call was made. With the buckets of every route's hour the point is the
 * group's own p95, worked out the way the period's is. Without them (a node that does not give them, or gave them to some
 * routes only) it is the slowest route's p95, and the group's own p95 is at most that: when every part of a whole has 95
 * of 100 within a time, so has the whole.
 */
export function trendOf(series: readonly LatencySeries[], hours: readonly number[], useBuckets = true): HourPoint[] {
  const byHour = new Map<number, HourSums>()
  for (const one of series) {
    for (const entry of one.byHour) {
      if (entry.count <= 0) continue
      const at = Date.parse(entry.hour)
      const known = byHour.get(at)
      const slowest = entry.p95Ms === undefined || known?.slowest === 'beyond' ? 'beyond' : known?.slowest === undefined ? entry.p95Ms : Math.max(known.slowest, entry.p95Ms)
      const buckets = entry.buckets === undefined || (known !== undefined && known.buckets === undefined) ? undefined : addBuckets(known?.buckets, entry.buckets)
      byHour.set(at, { count: (known?.count ?? 0 /* stand-in-ok(index): the first series of an hour has nothing of it yet, an accumulator of the code itself */) + entry.count, slowest, buckets })
    }
  }
  return hours.map((at) => pointOf(at, byHour.get(at), useBuckets))
}

const addBuckets = (sum: readonly number[] | undefined, more: readonly number[]): number[] => more.map((count, index) => count + (sum?.[index] ?? 0 /* stand-in-ok(index): the first series of an hour has no sum yet, an accumulator of the code itself */))

/** Whether any hour has a call: a range of more than 7 days comes with totals only, and a range of hours with none has nothing to draw. */
export const hasHours = (points: readonly HourPoint[]): boolean => points.some((point) => point.value.kind !== 'none')

/** Whether every hour with calls is the group's own p95. */
export const isExactTrend = (points: readonly HourPoint[]): boolean => points.every((point) => point.value.kind === 'none' || point.isExact)

/**
 * The trend of one chart, decided for the whole chart: the group's own p95 when every hour with calls has it, and otherwise
 * the slowest route's p95 for every hour, so that one line never mixes two meanings.
 */
export function trendFor(series: readonly LatencySeries[], hours: readonly number[]): { points: HourPoint[]; isExact: boolean } {
  const exact = trendOf(series, hours)
  return isExactTrend(exact) ? { points: exact, isExact: true } : { points: trendOf(series, hours, false), isExact: false }
}
