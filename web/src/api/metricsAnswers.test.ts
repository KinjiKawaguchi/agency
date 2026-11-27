import sample from '../../../testdata/metrics.sample.json' with { type: 'json' }
import { MalformedAnswer } from './evaluationAnswers'
import { readMetrics } from './metricsAnswers'

const buckets = (at: Record<number, number>): number[] => Array.from({ length: 15 }, (_, index) => at[index] ?? 0)
const series = (extra: Record<string, unknown> = {}) => ({ kind: 'serve', name: 'GET /api/me', status: '2xx', group: 'desk', isInNodeRead: true, count: 3, errors: 0, sumMs: 6, buckets: buckets({ 2: 3 }), p50Ms: 5, p95Ms: 5, p99Ms: 5, byHour: [], ...extra })
const answer = (extra: Record<string, unknown> = {}, list: unknown[] = [series()]) => ({ from: '2026-11-24T10:00:00Z', until: '2026-11-24T12:00:00Z', generatedAt: '2026-11-24T11:30:00Z', series: list, ...extra })

describe('reading the answer of the measurements route', () => {
  it('reads the shared sample', () => {
    const read = readMetrics(sample)
    expect(read.series).toHaveLength(sample.series.length)
    expect(read.series[0]).toMatchObject({ kind: 'serve', status: '2xx', count: sample.series[0]?.count })
  })

  it('reads the buckets of an hour when they are given, and has none when they are not', () => {
    const hour = { hour: '2026-11-24T10:00:00Z', count: 3, errors: 0, p95Ms: 5 }
    const [given] = readMetrics(answer({}, [series({ byHour: [{ ...hour, buckets: buckets({ 2: 3 }) }, { ...hour, hour: '2026-11-24T11:00:00Z' }] })])).series
    expect(given?.byHour[0]?.buckets).toEqual(buckets({ 2: 3 }))
    expect(given?.byHour[1]?.buckets).toBeUndefined()
    expect('buckets' in (given?.byHour[1] ?? {})).toBe(false)
  })

  it('refuses an hour whose buckets deny its count, or are not 15 numbers', () => {
    const hour = { hour: '2026-11-24T10:00:00Z', count: 3, errors: 0, p95Ms: 5 }
    for (const bad of [buckets({ 2: 2 }), buckets({ 2: 4 }), buckets({ 2: 3 }).slice(1), 'x', [...buckets({ 2: 3 }).slice(1), -0.5 + 0.5 - 1]]) {
      expect(() => readMetrics(answer({}, [series({ byHour: [{ ...hour, buckets: bad }] })]))).toThrow(MalformedAnswer)
    }
  })

  it('keeps a percentile that is left out as left out, and never fills it', () => {
    const [read] = readMetrics(answer({}, [series({ count: 4, buckets: buckets({ 14: 4 }), p50Ms: undefined, p95Ms: undefined, p99Ms: undefined })])).series
    expect(read?.p95Ms).toBeUndefined()
    expect('p95Ms' in (read ?? {})).toBe(false)
  })

  it('reads the budget and the objectives, and has none when the node says none', () => {
    const given = readMetrics(answer({ budget: { inNodeReadP95Ms: 100 }, objectives: [{ group: 'teams', p95Ms: 100, state: 'notMet', measuredP95Ms: 250 }, { group: 'desk', p95Ms: 50, state: 'later' }] }))
    expect(given.budgetInNodeReadP95Ms).toBe(100)
    expect(given.objectives).toEqual([{ group: 'teams', p95Ms: 100, state: 'notMet', measuredP95Ms: 250 }, { group: 'desk', p95Ms: 50, state: 'unknown' }])
    const none = readMetrics(answer())
    expect(none.budgetInNodeReadP95Ms).toBeUndefined()
    expect(none.objectives).toEqual([])
  })

  it('is in-node only for a route that says so, and a call or a client never is', () => {
    const read = readMetrics(answer({}, [series(), series({ isInNodeRead: undefined }), series({ kind: 'call', name: 'runtime view', group: 'runtime', isInNodeRead: true })])).series
    expect(read.map((one) => one.isInNodeRead)).toEqual([true, false, false])
  })

  it('leaves out a series of a kind it does not know, and has no hours for a range the node gave as totals', () => {
    const read = readMetrics(answer({}, [series({ kind: 'trace' }), series({ byHour: undefined })])).series
    expect(read).toHaveLength(1)
    expect(read[0]?.byHour).toEqual([])
  })

  it('refuses an answer whose buckets deny their count, or that has more errors than calls, or a number that is not one', () => {
    for (const bad of [series({ count: 4 }), series({ buckets: buckets({ 2: 3 }).slice(1) }), series({ errors: 4 }), series({ count: -3, buckets: buckets({ 2: -3 }) }), series({ p95Ms: '5' }), series({ sumMs: Number.NaN }), series({ group: '' })]) {
      expect(() => readMetrics(answer({}, [bad]))).toThrow(MalformedAnswer)
    }
    expect(() => readMetrics(answer({ from: 'yesterday' }))).toThrow(MalformedAnswer)
    expect(() => readMetrics({ ...answer(), series: 'none' })).toThrow(MalformedAnswer)
    expect(() => readMetrics(null)).toThrow(MalformedAnswer)
  })
})
