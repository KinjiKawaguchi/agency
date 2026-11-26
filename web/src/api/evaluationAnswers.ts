// The evaluation and effort answers come from the node, so they are read once, here, before any screen sees them:
// a type the node leaves out counts as 0, a number that is negative or not a number refuses the whole answer, and a
// list that is missing is empty. A screen can then rely on every type being there.
import { EFFORT_TYPES, PULL_REQUEST_TYPES } from './evaluationApi'
import type { DayEffort, DayEvaluation, Effort, EffortTokens, EffortType, Evaluation, EvaluationSummary, MergedCounts, TeamEffort, TeamEvaluation, TeamUnavailable } from './evaluationApi'

/** The answer is not what the route promises. The client turns it into a 503, "could not answer", shown in place. */
export class MalformedAnswer extends Error {
  constructor(what: string) {
    super(`The node’s answer has a bad ${what}.`)
    this.name = 'MalformedAnswer'
  }
}

type Raw = Record<string, unknown>

const asObject = (value: unknown): Raw => (typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Raw) : {})
const asList = (value: unknown): unknown[] => (Array.isArray(value) ? value : [])

/** A non-negative number; a missing one is 0. */
function amount(value: unknown, what: string): number {
  if (value === undefined || value === null) return 0
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) throw new MalformedAnswer(what)
  return value
}

/** A non-negative number, or `null` where the summary has nothing to say. */
const nullableAmount = (value: unknown, what: string): number | null => (value === undefined || value === null ? null : amount(value, what))

function text(value: unknown, what: string): string {
  if (typeof value !== 'string' || value === '') throw new MalformedAnswer(what)
  return value
}

const optionalText = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

function counts(raw: unknown, what: string): MergedCounts {
  const given = asObject(raw)
  return Object.fromEntries(PULL_REQUEST_TYPES.map((type) => [type, amount(given[type], `${what} ${type}`)])) as MergedCounts
}

function costs(raw: unknown, what: string): Record<EffortType, number> {
  const given = asObject(raw)
  return Object.fromEntries(EFFORT_TYPES.map((type) => [type, amount(given[type], `${what} ${type}`)])) as Record<EffortType, number>
}

const day = (raw: unknown): DayEvaluation => {
  const given = asObject(raw)
  return {
    date: text(given.date, 'date'),
    merged: counts(given.merged, 'merged'),
    reverts: amount(given.reverts, 'reverts'),
    mainRedSeconds: amount(given.mainRedSeconds, 'red time'),
    costUsd: amount(given.costUsd, 'cost'),
    prLeadSeconds: asList(given.prLeadSeconds).map((value) => amount(value, 'lead time')),
    issueLeadSeconds: asList(given.issueLeadSeconds).map((value) => amount(value, 'lead time')),
  }
}

function summary(raw: unknown): EvaluationSummary {
  const given = asObject(raw)
  return {
    merged: counts(given.merged, 'merged'),
    prLeadMedianSeconds: nullableAmount(given.prLeadMedianSeconds, 'lead time'),
    prLeadP90Seconds: nullableAmount(given.prLeadP90Seconds, 'lead time'),
    issueLeadMedianSeconds: nullableAmount(given.issueLeadMedianSeconds, 'lead time'),
    issueLeadP90Seconds: nullableAmount(given.issueLeadP90Seconds, 'lead time'),
    mainRedSeconds: amount(given.mainRedSeconds, 'red time'),
    longestRedSeconds: amount(given.longestRedSeconds, 'longest red time'),
    reverts: amount(given.reverts, 'reverts'),
    costPerMergedPrUsd: nullableAmount(given.costPerMergedPrUsd, 'cost'),
  }
}

const unavailable = (raw: unknown): TeamUnavailable[] =>
  asList(raw).map((one) => ({ team: text(asObject(one).team, 'team'), reason: typeof asObject(one).reason === 'string' ? (asObject(one).reason as string) : '' }))

function period(given: Raw): { org: string; days: number; generatedAt: string; from?: string; until?: string; unavailable: TeamUnavailable[] } {
  const from = optionalText(given.from)
  const until = optionalText(given.until)
  return {
    org: text(given.org, 'org'),
    days: amount(given.days, 'days'),
    generatedAt: text(given.generatedAt, 'time'),
    ...(from === undefined ? {} : { from }),
    ...(until === undefined ? {} : { until }),
    unavailable: unavailable(given.unavailable),
  }
}

export function readEvaluation(raw: unknown): Evaluation {
  const given = asObject(raw)
  const teams: TeamEvaluation[] = asList(given.teams).map((one) => {
    const team = asObject(one)
    return { team: text(team.team, 'team'), summary: summary(team.summary), byDay: asList(team.byDay).map(day) }
  })
  return { ...period(given), teams }
}

function tokens(raw: unknown): EffortTokens {
  const given = asObject(raw)
  return { input: amount(given.input, 'tokens'), output: amount(given.output, 'tokens'), cacheRead: amount(given.cacheRead, 'tokens'), cacheWrite: amount(given.cacheWrite, 'tokens') }
}

export function readEffort(raw: unknown): Effort {
  const given = asObject(raw)
  const teams: TeamEffort[] = asList(given.teams).map((one) => {
    const team = asObject(one)
    const byType = asObject(team.byType)
    const byDay: DayEffort[] = asList(team.byDay).map((entry) => ({ date: text(asObject(entry).date, 'date'), costUsd: costs(asObject(entry).costUsd, 'day cost') }))
    return {
      team: text(team.team, 'team'),
      byType: Object.fromEntries(EFFORT_TYPES.map((type) => [type, { costUsd: amount(asObject(byType[type]).costUsd, `${type} cost`), tokens: tokens(asObject(byType[type]).tokens) }])) as TeamEffort['byType'],
      byDay,
    }
  })
  return { ...period(given), teams }
}
