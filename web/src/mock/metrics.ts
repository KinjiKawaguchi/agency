import { BUCKET_COUNT } from '../api/metricsApi'

/** How the mock node's measurements look: the usual, nothing measured yet, or a node whose in-node reads are slow. */
export type MetricsVariant = 'usual' | 'empty' | 'slow'

interface Spec {
  kind: 'serve' | 'call'
  name: string
  group: string
  status: string
  isInNodeRead?: boolean
  peer?: string
  /** Durations as `[bucket index, how many]`. */
  at: ReadonlyArray<readonly [number, number]>
}

// A bucket is the upper bound of its index: 1 is 2 ms, 2 is 5 ms, 4 is 25 ms, 5 is 50 ms, 6 is 100 ms, 7 is 250 ms, 9 is 1 s, 11 is 5 s, and 14 is slower than 30 s.
const usual: Spec[] = [
  { kind: 'serve', name: 'GET /api/me', group: 'desk', status: '2xx', isInNodeRead: true, at: [[1, 3200], [2, 800], [4, 40]] },
  { kind: 'serve', name: 'GET /api/orgs/{org}/teams/{team}/view', group: 'teams', status: '2xx', isInNodeRead: true, at: [[2, 900], [4, 2600], [5, 380], [6, 90], [7, 6]] },
  { kind: 'serve', name: 'GET /api/orgs/{org}/teams/{team}/view', group: 'teams', status: '5xx', isInNodeRead: true, at: [[9, 4]] },
  { kind: 'serve', name: 'GET /api/orgs/{org}/teams/{team}/activity', group: 'teams', status: '2xx', at: [[7, 120], [9, 60], [11, 8]] },
  { kind: 'serve', name: 'GET /api/orgs/{org}/teams/{team}/usage', group: 'usage', status: '2xx', at: [[4, 30], [9, 60], [11, 10], [14, 2]] },
  { kind: 'serve', name: 'GET /api/orgs/{org}/evaluation', group: 'evaluation', status: '2xx', at: [[9, 40], [11, 12]] },
  { kind: 'serve', name: 'POST /api/devices', group: 'devices', status: '2xx', at: [[3, 12], [4, 3]] },
  { kind: 'call', name: 'runtime view', group: 'runtime', status: 'ok', at: [[7, 800], [9, 160], [11, 9]] },
  { kind: 'call', name: 'peer get_view', group: 'peer', status: '2xx', peer: 'node-b.example', at: [[5, 40], [6, 60], [9, 5]] },
  { kind: 'call', name: 'apns send', group: 'push', status: '2xx', at: [[6, 30], [7, 20]] },
]

const slow: Spec[] = usual.map((spec) => (spec.isInNodeRead === true && spec.status === '2xx' && spec.group === 'teams' ? { ...spec, at: [[5, 100], [7, 700], [9, 200]] } : spec))

const bucketsOf = (at: Spec['at']): number[] => {
  const buckets = Array.from({ length: BUCKET_COUNT }, () => 0)
  for (const [index, count] of at) buckets[index] = (buckets[index] ?? 0) + count
  return buckets
}

const BOUNDS = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10_000, 30_000]

function percentile(buckets: readonly number[], percent: number): number | undefined {
  const total = buckets.reduce((sum, one) => sum + one, 0)
  const rank = Math.max(1, Math.floor((total * percent + 99) / 100))
  let seen = 0
  for (const [index, count] of buckets.entries()) {
    seen += count
    if (seen >= rank) return BOUNDS[index]
  }
  return undefined
}

/**
 * The hours of a series: each hour of the range has its share of the calls, and its p95 is the series' own, or a bound
 * next to it, so that the line moves; some hours have no call (a gap), and a series whose p95 is beyond the last bucket
 * has hours of its own that are. A range of more than 7 days has no hours, as the node gives totals only.
 */
function hoursOf(spec: Spec, count: number, until: Date, days: number, p95: number | undefined): unknown[] {
  if (days > 7) return []
  const hours = days * 24
  const base = p95 === undefined ? BOUNDS.length - 1 : BOUNDS.indexOf(p95)
  const wobble = [0, 0, 1, 0, -1, 0, 0, 2, 0, -1]
  return Array.from({ length: hours }, (_, index) => {
    const isGap = (index * 7 + spec.name.length) % 11 === 0
    if (isGap) return undefined
    const calls = Math.max(1, Math.round(count / hours))
    const shifted = Math.min(BOUNDS.length - 1, Math.max(0, base + (wobble[index % wobble.length] ?? 0)))
    const isBeyond = p95 === undefined && index % 5 === 0
    return {
      hour: new Date(until.getTime() - (hours - index) * 3_600_000).toISOString(),
      count: calls,
      errors: spec.status === '5xx' ? calls : 0,
      ...(isBeyond ? {} : { p95Ms: BOUNDS[shifted] }),
    }
  }).filter((hour) => hour !== undefined)
}

/** The owner's measurements as the node answers them: the series of a few route groups and calls, from the hour before `now` back `days` days. */
export function metricsAnswer(now: Date, days: number, variant: MetricsVariant): unknown {
  const until = new Date(Math.floor(now.getTime() / 3_600_000) * 3_600_000 + 3_600_000)
  const from = new Date(until.getTime() - days * 24 * 3_600_000)
  const specs = variant === 'empty' ? [] : variant === 'slow' ? slow : usual
  const series = specs.map((spec) => {
    const buckets = bucketsOf(spec.at)
    const count = buckets.reduce((sum, one) => sum + one, 0)
    const isError = spec.status === '5xx'
    return {
      kind: spec.kind,
      name: spec.name,
      status: spec.status,
      group: spec.group,
      ...(spec.peer === undefined ? {} : { peer: spec.peer }),
      ...(spec.kind === 'serve' ? { isInNodeRead: spec.isInNodeRead === true } : {}),
      count,
      errors: isError ? count : 0,
      sumMs: 0,
      buckets,
      ...Object.fromEntries(([['p50Ms', 50], ['p95Ms', 95], ['p99Ms', 99]] as const).flatMap(([key, percent]) => {
        const found = percentile(buckets, percent)
        return found === undefined ? [] : [[key, found]]
      })),
      byHour: hoursOf(spec, count, until, days, percentile(buckets, 95)),
    }
  })
  return {
    from: from.toISOString(),
    until: until.toISOString(),
    generatedAt: now.toISOString(),
    budget: { inNodeReadP95Ms: 100 },
    objectives: variant === 'empty' ? [] : [
      { group: 'teams', p95Ms: 100, state: variant === 'slow' ? 'notMet' : 'met', measuredP95Ms: variant === 'slow' ? 1000 : 100 },
      { group: 'desk', p95Ms: 50, state: 'met', measuredP95Ms: 5 },
      { group: 'evaluation', p95Ms: 2000, state: 'unknown' },
    ],
    series,
  }
}
