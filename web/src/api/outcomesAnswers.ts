// The outcomes and readings answers come from the node, so they are read once, here, before any screen sees them. A number
// that is not a finite number, a list that is not a list, or a day that is not a day refuses the whole answer (the
// client turns it into a 503, "could not answer"); a value this client does not know (a unit, a `goodWhen`, a state)
// does not: it is kept as unknown. Nothing is ever filled in: a missing number stays missing.
import { MalformedAnswer } from './evaluationAnswers'
import { READING_STATES, type KpiInfo, type KpiSeries, type KpiUnit, type NumberOnDay, type Outcomes, type Reading, type ReadingStateName, type Readings } from './outcomesApi'

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

const DAY = /^\d{4}-\d{2}-\d{2}$/
function day(value: unknown, what: string): string {
  const found = text(value, what)
  // A day of the calendar: 2026-13-45 and 2026-02-30 have the shape but are not days, and would draw as an invalid date.
  const parsed = new Date(`${found}T00:00:00Z`)
  if (!DAY.test(found) || Number.isNaN(parsed.getTime()) || parsed.toISOString().slice(0, 10) !== found) throw new MalformedAnswer(what)
  return found
}

function figure(value: unknown, what: string): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new MalformedAnswer(what)
  return value
}

const optional = <T>(value: unknown, read: (value: unknown) => T): T | undefined => (value === undefined || value === null ? undefined : read(value))

function numberOnDay(raw: unknown, what: string): NumberOnDay {
  const given = asObject(raw, what)
  const note = optional(given.note, (value) => text(value, `${what} note`))
  return { date: day(given.date, `${what} date`), value: figure(given.value, `${what} value`), ...(note === undefined ? {} : { note }) }
}

const UNITS: readonly KpiUnit[] = ['money', 'percent', 'count', 'ratio', 'seconds']

function kpi(raw: unknown): KpiInfo {
  const given = asObject(raw, 'KPI')
  const unit = UNITS.find((one) => one === given.unit) ?? 'unknown'
  const currency = unit === 'money' ? optional(given.currency, (value) => text(value, 'currency')) : undefined
  const goodWhen = given.goodWhen === 'higher' || given.goodWhen === 'lower' ? given.goodWhen : undefined
  return {
    id: text(given.id, 'KPI id'),
    label: text(given.label, 'KPI label'),
    kind: typeof given.kind === 'string' ? given.kind : '',
    unit,
    ...(currency === undefined ? {} : { currency }),
    ...(goodWhen === undefined ? {} : { goodWhen }),
  }
}

function series(raw: unknown): KpiSeries {
  const given = asObject(raw, 'series')
  const before = optional(given.before, (value) => numberOnDay(value, 'baseline'))
  return { kpi: kpi(given.kpi), points: asList(given.points, 'points').map((point) => numberOnDay(point, 'point')), ...(before === undefined ? {} : { before }) }
}

export function readOutcomes(raw: unknown): Outcomes {
  const given = asObject(raw, 'answer')
  const days = figure(given.days, 'days')
  return { org: text(given.org, 'org'), days, from: day(given.from, 'from'), until: day(given.until, 'until'), generatedAt: text(given.generatedAt, 'time'), series: asList(given.series, 'series').map(series) }
}

function reading(raw: unknown): Reading {
  const given = asObject(raw, 'reading')
  const state: ReadingStateName | 'unknown' = READING_STATES.find((one) => one === given.state) ?? 'unknown'
  const isReady = state === 'ready'
  const epicTitle = optional(given.epicTitle, (value) => text(value, 'epic title'))
  const shippedOn = optional(given.shippedOn, (value) => day(value, 'ship day'))
  const readableFrom = optional(given.readableFrom, (value) => day(value, 'readable day'))
  const points = optional(given.points, (value) => figure(value, 'points'))
  const needs = optional(given.needs, (value) => figure(value, 'needs'))
  // Numbers are the node's only in `ready`; in any other state a number that came with the reading is not drawn.
  const before = isReady ? optional(given.before, (value) => numberOnDay(value, 'before')) : undefined
  const after = isReady ? optional(given.after, (value) => numberOnDay(value, 'after')) : undefined
  const change = isReady ? optional(given.change, (value) => figure(value, 'change')) : undefined
  const changeRatio = isReady ? optional(given.changeRatio, (value) => figure(value, 'change ratio')) : undefined
  return {
    team: text(given.team, 'team'),
    epic: text(given.epic, 'epic'),
    kpi: text(given.kpi, 'KPI'),
    state,
    overlapping: isReady ? asList(given.overlapping, 'overlapping').map((one) => text(one, 'overlapping epic')) : [],
    ...(epicTitle === undefined ? {} : { epicTitle }),
    ...(shippedOn === undefined ? {} : { shippedOn }),
    ...(readableFrom === undefined ? {} : { readableFrom }),
    ...(points === undefined ? {} : { points }),
    ...(needs === undefined ? {} : { needs }),
    ...(before === undefined ? {} : { before }),
    ...(after === undefined ? {} : { after }),
    ...(change === undefined ? {} : { change }),
    ...(changeRatio === undefined ? {} : { changeRatio }),
  }
}

export function readReadings(raw: unknown): Readings {
  const given = asObject(raw, 'answer')
  return { org: text(given.org, 'org'), generatedAt: text(given.generatedAt, 'time'), readings: asList(given.readings, 'readings').map(reading) }
}
