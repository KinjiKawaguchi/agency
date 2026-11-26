// The outcomes routes of the Client API: the numbers an org's KPIs have (owner only) and the readings of how they
// stand around the day an aim's epic shipped (docs/protocol.md, "Outcomes"). The types of the answers are generated
// from the Rust types in `bindings/`; the ones here are what the screen works with once `outcomesAnswers.ts` has read
// an answer: a value this client does not know (a unit, a `goodWhen`, a state) is kept as unknown and never fails the whole.
export type { Observation } from '../../../bindings/Observation'

/** What a KPI's number counts. `unknown` is a unit a newer node may add: its numbers are shown as plain numbers. */
export type KpiUnit = 'money' | 'percent' | 'count' | 'ratio' | 'seconds' | 'unknown'

export interface KpiInfo {
  id: string
  label: string
  kind: string
  unit: KpiUnit
  /** ISO 4217, with `money` only. */
  currency?: string
  /** Which way is better; left out when the node says something this client does not know: the movement is then not coloured. */
  goodWhen?: 'higher' | 'lower'
}

/** One number of a KPI on one day. */
export interface NumberOnDay {
  date: string
  value: number
  note?: string
}

export interface KpiSeries {
  kpi: KpiInfo
  /** Only the days that have a number, oldest first. A day with none is a gap, never a 0. */
  points: NumberOnDay[]
  /** The last number before the period: the baseline a movement is read from. */
  before?: NumberOnDay
}

export interface Outcomes {
  org: string
  days: number
  from: string
  until: string
  generatedAt: string
  series: KpiSeries[]
}

export const READING_STATES = ['ready', 'notShipped', 'shipDateUnknown', 'noBaseline', 'tooEarly', 'noFollowUp', 'tooFew'] as const
export type ReadingStateName = (typeof READING_STATES)[number]

export interface Reading {
  team: string
  epic: string
  epicTitle?: string
  kpi: string
  shippedOn?: string
  /** `unknown` is a state a newer node may add: the reading says it cannot be read here, and shows no numbers. */
  state: ReadingStateName | 'unknown'
  points?: number
  needs?: number
  readableFrom?: string
  /** Numbers exist only with `ready`. */
  before?: NumberOnDay
  after?: NumberOnDay
  change?: number
  changeRatio?: number
  overlapping: string[]
}

export interface Readings {
  org: string
  generatedAt: string
  readings: Reading[]
}
