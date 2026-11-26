import sample from '../../../testdata/metrics.sample.json' with { type: 'json' }
import { readMetrics } from '../api/metricsAnswers'
import type { LatencySeries } from '../api/metricsApi'
import { groupsOf, inNodeReads, percentileOf, readsOverBudget, rowOf, verdictOf } from './latency'

const buckets = (at: Record<number, number>): number[] => Array.from({ length: 15 }, (_, index) => at[index] ?? 0)
const series = (overrides: Partial<LatencySeries> & { buckets: number[] }): LatencySeries => ({
  kind: 'serve',
  name: 'GET /api/me',
  status: '2xx',
  group: 'desk',
  isInNodeRead: true,
  count: overrides.buckets.reduce((sum, one) => sum + one, 0),
  errors: 0,
  sumMs: 0,
  byHour: [],
  ...overrides,
})

describe('a percentile of buckets, as the node reads it', () => {
  it('is the upper bound of the bucket of the nearest rank', () => {
    // 10 durations: ranks 5 (p50), 10 (p95), 10 (p99).
    expect(percentileOf(buckets({ 0: 5, 4: 4, 9: 1 }), 50)).toEqual({ kind: 'atMost', ms: 1 })
    expect(percentileOf(buckets({ 0: 5, 4: 4, 9: 1 }), 95)).toEqual({ kind: 'atMost', ms: 1000 })
    expect(percentileOf(buckets({ 0: 5, 4: 4, 9: 1 }), 90)).toEqual({ kind: 'atMost', ms: 25 })
  })

  it('does not move a rank by a rounding of a product: 20 durations at p95 is the 19th, not the 20th', () => {
    expect(percentileOf(buckets({ 6: 19, 9: 1 }), 95)).toEqual({ kind: 'atMost', ms: 100 })
    expect(percentileOf(buckets({ 6: 18, 9: 2 }), 95)).toEqual({ kind: 'atMost', ms: 1000 })
  })

  it('is beyond the last bound for what fell in the last bucket, and nothing for no durations', () => {
    expect(percentileOf(buckets({ 8: 3, 14: 1 }), 99)).toEqual({ kind: 'beyond' })
    expect(percentileOf(buckets({ 8: 3, 14: 1 }), 50)).toEqual({ kind: 'atMost', ms: 500 })
    expect(percentileOf(buckets({}), 95)).toBeUndefined()
  })

  it('agrees with the node on every series of the shared sample', () => {
    for (const one of readMetrics(sample).series) {
      for (const [percent, given] of [[50, one.p50Ms], [95, one.p95Ms], [99, one.p99Ms]] as const) {
        const own = percentileOf(one.buckets, percent)
        expect(own?.kind === 'atMost' ? own.ms : undefined, `${one.name} ${one.status} p${percent}`).toBe(given)
      }
    }
  })
})

describe('the rows by route group', () => {
  const all = [
    series({ name: 'GET /api/me', group: 'desk', buckets: buckets({ 2: 6 }) }),
    series({ name: 'GET /api/me', status: '5xx', group: 'desk', errors: 2, buckets: buckets({ 12: 2 }) }),
    series({ name: 'GET /api/orgs/{org}/teams/{team}/view', group: 'teams', buckets: buckets({ 4: 3 }) }),
    series({ name: 'GET /api/orgs/{org}/teams/{team}/usage', group: 'usage', isInNodeRead: false, buckets: buckets({ 9: 1 }) }),
    series({ kind: 'call', name: 'peer get_view', group: 'peer', peer: 'node-b.example', isInNodeRead: false, buckets: buckets({ 7: 2 }) }),
  ]

  it('puts the statuses of a route together, adds the buckets, and does not average the percentiles', () => {
    const desk = groupsOf(all, 'serve').find((row) => row.key === 'desk')
    expect(desk).toMatchObject({ count: 8, errors: 2 })
    expect(desk?.routes).toHaveLength(1)
    // 8 durations: p50 is the 4th (5 ms bucket); p95 the 8th, in the 10 000 ms bucket.
    expect(desk?.p50).toEqual({ kind: 'atMost', ms: 5 })
    expect(desk?.p95).toEqual({ kind: 'atMost', ms: 10_000 })
  })

  it('lists the busiest group first and keeps the kinds apart', () => {
    expect(groupsOf(all, 'serve').map((row) => row.key)).toEqual(['desk', 'teams', 'usage'])
    expect(groupsOf(all, 'call').map((row) => row.key)).toEqual(['peer'])
    expect(groupsOf(all, 'call')[0]?.routes[0]?.key).toBe('peer get_view (node-b.example)')
  })

  it('leaves out a series with no call, and a kind with none has no rows', () => {
    expect(groupsOf([series({ buckets: buckets({}) })], 'serve')).toEqual([])
    expect(groupsOf(all, 'client')).toEqual([])
  })

  it('puts only the in-node reads under the budget', () => {
    const reads = inNodeReads(all)
    expect(reads?.count).toBe(11)
    expect(inNodeReads([all[3]!])).toBeUndefined()
    expect(readsOverBudget(all, 100).map((row) => row.key)).toEqual(['GET /api/me'])
  })
})

describe('the budget is a test of the p95 against a bound', () => {
  it('is within at the bound and over just above it, or beyond the last bucket, and says nothing for no durations', () => {
    expect(verdictOf({ kind: 'atMost', ms: 100 }, 100)).toEqual({ kind: 'within' })
    expect(verdictOf({ kind: 'atMost', ms: 250 }, 100)).toEqual({ kind: 'over' })
    expect(verdictOf({ kind: 'beyond' }, 100)).toEqual({ kind: 'over' })
    expect(verdictOf(undefined, 100)).toBeUndefined()
  })

  it('cannot tell, when the budget falls inside the bucket the p95 lies in: 60 ms may be within 75 ms and 90 ms is not', () => {
    expect(verdictOf({ kind: 'atMost', ms: 100 }, 75)).toEqual({ kind: 'undetermined' })
    expect(verdictOf({ kind: 'atMost', ms: 50 }, 75)).toEqual({ kind: 'within' })
    expect(verdictOf({ kind: 'atMost', ms: 250 }, 75)).toEqual({ kind: 'over' })
    expect(verdictOf({ kind: 'atMost', ms: 250 }, 100)).toEqual({ kind: 'over' })
    expect(verdictOf({ kind: 'atMost', ms: 250 }, 175)).toEqual({ kind: 'undetermined' })
    expect(verdictOf({ kind: 'atMost', ms: 1 }, 1)).toEqual({ kind: 'within' })
    expect(verdictOf({ kind: 'beyond' }, 30_000)).toEqual({ kind: 'over' })
    expect(verdictOf({ kind: 'beyond' }, 40_000)).toEqual({ kind: 'undetermined' })
  })

  it('lists no read as over the budget that may be within it', () => {
    const reads = [series({ name: 'GET /api/me', buckets: buckets({ 6: 10 }) })]
    expect(readsOverBudget(reads, 75)).toEqual([])
    expect(readsOverBudget(reads, 50).map((row) => row.key)).toEqual(['GET /api/me'])
  })

  it('reads a row of one series like the node does', () => {
    expect(rowOf('x', [series({ buckets: buckets({ 6: 20 }) })]).p95).toEqual({ kind: 'atMost', ms: 100 })
  })
})
