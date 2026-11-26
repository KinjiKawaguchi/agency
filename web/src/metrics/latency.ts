// What the screen works out of the node's series: the percentile of several series put together, the rows by route
// group and by route, and whether in-node reads keep the budget. A percentile is always the upper bound of a bucket,
// as the node gives it, and a series' own percentiles are never averaged: buckets are added, and the percentile is read
// from the sum, as the node reads it from one.
import { BUCKET_BOUNDS_MS, BUCKET_COUNT, type LatencySeries } from '../api/metricsApi'

/** A percentile: `atMost` the upper bound of its bucket, or `beyond` the last one (slower than the last bound). */
export type Bound = { kind: 'atMost'; ms: number } | { kind: 'beyond' }

/** Nearest rank, as the node reads it: the `ceil(n x percent / 100)`-th duration, given as its bucket's upper bound. Nothing when there are no durations. */
export function percentileOf(buckets: readonly number[], percent: number): Bound | undefined {
  const total = buckets.reduce((sum, one) => sum + one, 0)
  if (total === 0) return undefined
  // Integers all the way: no rounding of a product like 19.000000000000004 can move the rank.
  const rank = Math.max(1, Math.floor((total * percent + 99) / 100))
  let seen = 0
  for (const [index, inBucket] of buckets.entries()) {
    seen += inBucket
    if (seen >= rank) {
      const bound = BUCKET_BOUNDS_MS[index]
      return bound === undefined ? { kind: 'beyond' } : { kind: 'atMost', ms: bound }
    }
  }
  return { kind: 'beyond' }
}

export interface Row {
  /** What the row is named: a route group, or a route template, or a call. */
  key: string
  count: number
  errors: number
  p50: Bound | undefined
  p95: Bound | undefined
  p99: Bound | undefined
}

export interface GroupRow extends Row {
  routes: Row[]
}

const sumBuckets = (series: readonly LatencySeries[]): number[] => {
  const sum = Array.from({ length: BUCKET_COUNT }, () => 0)
  for (const one of series) for (const [index, inBucket] of one.buckets.entries()) sum[index] = (sum[index] ?? 0 /* stand-in-ok(index): the sum has a place for every bucket of a series */) + inBucket
  return sum
}

export function rowOf(key: string, series: readonly LatencySeries[]): Row {
  const buckets = sumBuckets(series)
  return {
    key,
    count: series.reduce((sum, one) => sum + one.count, 0),
    errors: series.reduce((sum, one) => sum + one.errors, 0),
    p50: percentileOf(buckets, 50),
    p95: percentileOf(buckets, 95),
    p99: percentileOf(buckets, 99),
  }
}

const byKey = (rows: readonly Row[]): Row[] => [...rows].sort((a, b) => b.count - a.count || a.key.localeCompare(b.key))

/** A call is told apart by the peer it went to; a route has no peer. */
const routeKey = (one: LatencySeries): string => (one.peer === undefined ? one.name : `${one.name} (${one.peer})`)

function groupBy<T>(items: readonly T[], keyOf: (item: T) => string): Map<string, T[]> {
  const groups = new Map<string, T[]>()
  for (const item of items) groups.set(keyOf(item), [...(groups.get(keyOf(item)) ?? [] /* stand-in-ok(index): the first of a group has no list yet, an accumulator of the code itself */), item])
  return groups
}

/** The series of one kind by route group, each group with its routes (statuses and clients put together), busiest first. A series with no call is not a row. */
export function groupsOf(series: readonly LatencySeries[], kind: LatencySeries['kind']): GroupRow[] {
  const measured = series.filter((one) => one.kind === kind && one.count > 0)
  const rows = [...groupBy(measured, (one) => one.group)].map(([group, members]) => ({
    ...rowOf(group, members),
    routes: byKey([...groupBy(members, routeKey)].map(([key, parts]) => rowOf(key, parts))),
  }))
  return rows.sort((a, b) => b.count - a.count || a.key.localeCompare(b.key))
}

export type Verdict = { kind: 'within' } | { kind: 'over' }

/** The budget is a test of the p95 against a bound: `p95Ms <= budget`. A p95 beyond the last bucket is over it. */
export function verdictOf(p95: Bound | undefined, budgetMs: number): Verdict | undefined {
  if (p95 === undefined) return undefined
  return p95.kind === 'atMost' && p95.ms <= budgetMs ? { kind: 'within' } : { kind: 'over' }
}

/** Every in-node read put together, for the budget; nothing when none was measured. */
export function inNodeReads(series: readonly LatencySeries[]): Row | undefined {
  const reads = series.filter((one) => one.kind === 'serve' && one.isInNodeRead && one.count > 0)
  return reads.length === 0 ? undefined : rowOf('in-node reads', reads)
}

/** Each in-node read route's own row, with the verdict: which of them breaks the budget. */
export function readsOverBudget(series: readonly LatencySeries[], budgetMs: number): Row[] {
  const reads = series.filter((one) => one.kind === 'serve' && one.isInNodeRead && one.count > 0)
  return byKey([...groupBy(reads, routeKey)].map(([key, parts]) => rowOf(key, parts))).filter((row) => verdictOf(row.p95, budgetMs)?.kind === 'over')
}
