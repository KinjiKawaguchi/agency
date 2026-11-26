// The metrics answer comes from the node, so it is read once, here, before any screen sees it. A number that is not a
// finite non-negative number, a list that is not a list, or a series whose buckets do not add up to its count refuses
// the whole answer (the client turns it into a 503): a screen must never show a percentile that its own buckets deny.
// A value this client does not know (a kind of series, an objective's state) is kept as unknown, or left out where the
// screen could not place it. Nothing is ever filled in: a missing percentile stays missing.
import { MalformedAnswer } from './evaluationAnswers'
import { BUCKET_COUNT, type HourSeries, type LatencySeries, type Metrics, type Objective, type ObjectiveState, type SeriesKind } from './metricsApi'

type Raw = Record<string, unknown>

const asObject = (value: unknown, what: string): Raw => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new MalformedAnswer(what)
  return value as Raw
}
const asList = (value: unknown, what: string): unknown[] => {
  if (!Array.isArray(value)) throw new MalformedAnswer(what)
  return value
}
function text(value: unknown, what: string): string {
  if (typeof value !== 'string' || value === '') throw new MalformedAnswer(what)
  return value
}
function time(value: unknown, what: string): string {
  const found = text(value, what)
  if (Number.isNaN(Date.parse(found))) throw new MalformedAnswer(what)
  return found
}
function count(value: unknown, what: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) throw new MalformedAnswer(what)
  return value
}
const optional = <T>(value: unknown, read: (value: unknown) => T): T | undefined => (value === undefined || value === null ? undefined : read(value))

const KINDS: readonly SeriesKind[] = ['serve', 'call', 'client']

function hour(raw: unknown): HourSeries {
  const given = asObject(raw, 'hour')
  const p95Ms = optional(given.p95Ms, (value) => count(value, 'hour p95'))
  return { hour: time(given.hour, 'hour'), count: count(given.count, 'hour count'), errors: count(given.errors, 'hour errors'), ...(p95Ms === undefined ? {} : { p95Ms }) }
}

/** A series of a kind this client does not know is left out (null): it could not be placed in any table. */
function series(raw: unknown): LatencySeries | null {
  const given = asObject(raw, 'series')
  const kind = KINDS.find((one) => one === given.kind)
  if (kind === undefined) return null
  const buckets = asList(given.buckets, 'buckets').map((one) => count(one, 'bucket'))
  const total = count(given.count, 'count')
  if (buckets.length !== BUCKET_COUNT || buckets.reduce((sum, one) => sum + one, 0) !== total) throw new MalformedAnswer('buckets')
  const errors = count(given.errors, 'errors')
  if (errors > total) throw new MalformedAnswer('errors')
  const peer = optional(given.peer, (value) => text(value, 'peer'))
  const client = optional(given.client, (value) => text(value, 'client'))
  const [p50Ms, p95Ms, p99Ms] = (['p50Ms', 'p95Ms', 'p99Ms'] as const).map((key) => optional(given[key], (value) => count(value, key)))
  return {
    kind,
    name: text(given.name, 'name'),
    status: text(given.status, 'status'),
    group: text(given.group, 'group'),
    ...(peer === undefined ? {} : { peer }),
    ...(client === undefined ? {} : { client }),
    isInNodeRead: kind === 'serve' && given.isInNodeRead === true,
    count: total,
    errors,
    sumMs: count(given.sumMs, 'sum'),
    buckets,
    ...(p50Ms === undefined ? {} : { p50Ms }),
    ...(p95Ms === undefined ? {} : { p95Ms }),
    ...(p99Ms === undefined ? {} : { p99Ms }),
    byHour: asList(given.byHour ?? [] /* stand-in-ok(optional): a range of more than 7 days comes with totals only, and the node leaves the hours out */, 'hours').map(hour),
  }
}

/** The states this client knows: a new one in the generated type is a compile error here, to be worded and drawn. */
const KNOWN_STATES: Record<ObjectiveState, true> = { met: true, notMet: true, undetermined: true, unknown: true }

const stateOf = (value: unknown): ObjectiveState => (typeof value === 'string' && Object.hasOwn(KNOWN_STATES, value) ? (value as ObjectiveState) : 'unknown')

function objective(raw: unknown): Objective {
  const given = asObject(raw, 'objective')
  const measuredP95Ms = optional(given.measuredP95Ms, (value) => count(value, 'measured p95'))
  return {
    group: text(given.group, 'objective group'),
    p95Ms: count(given.p95Ms, 'objective p95'),
    state: stateOf(given.state),
    ...(measuredP95Ms === undefined ? {} : { measuredP95Ms }),
  }
}

export function readMetrics(raw: unknown): Metrics {
  const given = asObject(raw, 'answer')
  const budget = optional(given.budget, (value) => count(asObject(value, 'budget').inNodeReadP95Ms, 'budget'))
  return {
    from: time(given.from, 'from'),
    until: time(given.until, 'until'),
    generatedAt: time(given.generatedAt, 'time'),
    ...(budget === undefined ? {} : { budgetInNodeReadP95Ms: budget }),
    objectives: asList(given.objectives ?? [] /* stand-in-ok(optional): a node with no objective set leaves the list out */, 'objectives').map(objective),
    series: asList(given.series, 'series').flatMap((one) => series(one) ?? []),
  }
}
