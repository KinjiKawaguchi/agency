// The evaluation and effort answers come from the node, so they are read once, here, before any screen sees them:
// a type the node leaves out counts as 0, a number that is negative or not a number refuses the whole answer, and a
// list that is missing is empty, and the period each team names is required, since its days are drawn against it. An
// optional field the node leaves out stays left out. A screen can then rely on every type being there.
import { EFFORT_TYPES, PULL_REQUEST_TYPES } from './evaluationApi'
import type { TokenCounts } from './clientApi'
import type { DayEffort, DayEvaluation, Effort, EffortType, Evaluation, EvaluationSummary, MergedCounts, NewFeatureSummary, ParticipantEvaluation, RepositoryEvaluation, TeamEffort, TeamEvaluation, TeamUnavailable } from './evaluationApi'

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

/** A non-negative number, or `undefined` where the summary has nothing to say; the node leaves such a field out, and a null is read the same. */
const optionalAmount = (value: unknown, what: string): number | undefined => (value === undefined || value === null ? undefined : amount(value, what))

/** The field as a property that is there only when there is something in it. */
const present = <K extends string>(key: K, value: number | undefined): { [P in K]?: number } => (value === undefined ? {} : ({ [key]: value } as { [P in K]?: number }))

function text(value: unknown, what: string): string {
  if (typeof value !== 'string' || value === '') throw new MalformedAnswer(what)
  return value
}

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

/** The lead times of the period's new features alone; the node leaves the part out when none merged, and so does this. */
function newFeatureSummary(raw: unknown): NewFeatureSummary {
  const given = asObject(raw)
  return {
    merged: amount(given.merged, 'new features merged'),
    ...present('prLeadMedianSeconds', optionalAmount(given.prLeadMedianSeconds, 'lead time')),
    ...present('prLeadP90Seconds', optionalAmount(given.prLeadP90Seconds, 'lead time')),
    ...present('issueLeadMedianSeconds', optionalAmount(given.issueLeadMedianSeconds, 'lead time')),
    ...present('issueLeadP90Seconds', optionalAmount(given.issueLeadP90Seconds, 'lead time')),
  }
}

function summary(raw: unknown): EvaluationSummary {
  const given = asObject(raw)
  const newFeature = given.newFeature === undefined || given.newFeature === null ? {} : { newFeature: newFeatureSummary(given.newFeature) }
  return {
    merged: counts(given.merged, 'merged'),
    ...present('prLeadMedianSeconds', optionalAmount(given.prLeadMedianSeconds, 'lead time')),
    ...present('prLeadP90Seconds', optionalAmount(given.prLeadP90Seconds, 'lead time')),
    ...present('issueLeadMedianSeconds', optionalAmount(given.issueLeadMedianSeconds, 'lead time')),
    ...present('issueLeadP90Seconds', optionalAmount(given.issueLeadP90Seconds, 'lead time')),
    mainRedSeconds: amount(given.mainRedSeconds, 'red time'),
    longestRedSeconds: amount(given.longestRedSeconds, 'longest red time'),
    reverts: amount(given.reverts, 'reverts'),
    ...present('costPerMergedPrUsd', optionalAmount(given.costPerMergedPrUsd, 'cost')),
    ...newFeature,
  }
}

const unavailable = (raw: unknown): TeamUnavailable[] =>
  asList(raw).map((one) => ({ team: text(asObject(one).team, 'team'), reason: typeof asObject(one).reason === 'string' ? (asObject(one).reason as string) : '' }))

/** The period a report or a team names: its first and last calendar day, in the time zone of the node that answers. */
const periodOf = (given: Raw): { from: string; until: string } => ({ from: text(given.from, 'first day'), until: text(given.until, 'last day') }) // shown-ok(period): the first day of a range

function header(given: Raw): { org: string; days: number; generatedAt: string; from: string; until: string; unavailable: TeamUnavailable[] } {
  return { org: text(given.org, 'org'), days: amount(given.days, 'days'), generatedAt: text(given.generatedAt, 'time'), ...periodOf(given), unavailable: unavailable(given.unavailable) }
}

/** One participant's numbers; the node leaves `costPerMergedPrUsd` out when they merged nothing. */
function participant(raw: unknown): ParticipantEvaluation {
  const given = asObject(raw)
  return {
    name: text(given.name, 'participant'),
    busySeconds: amount(given.busySeconds, 'busy time'),
    waitingSeconds: amount(given.waitingSeconds, 'waiting time'),
    idleSeconds: amount(given.idleSeconds, 'idle time'),
    costUsd: amount(given.costUsd, 'cost'),
    tokens: tokens(given.tokens),
    merged: amount(given.merged, 'merged'),
    ...present('costPerMergedPrUsd', optionalAmount(given.costPerMergedPrUsd, 'cost')),
  }
}

/** One repository's summary; it has no cost, and the node leaves one out. */
function repository(raw: unknown): RepositoryEvaluation {
  const given = asObject(raw)
  return { repo: text(given.repo, 'repository'), summary: summary(given.summary) }
}

export function readEvaluation(raw: unknown): Evaluation {
  const given = asObject(raw)
  const teams: TeamEvaluation[] = asList(given.teams).map((one) => {
    const team = asObject(one)
    const participants = asList(team.participants).map(participant)
    const byRepository = asList(team.byRepository).map(repository)
    return {
      team: text(team.team, 'team'),
      ...periodOf(team),
      summary: summary(team.summary),
      byDay: asList(team.byDay).map(day),
      // Only the team's owner is given them: for anyone else they are left out, and stay left out here.
      ...(participants.length > 0 ? { participants } : {}),
      ...present('unattributedMerged', optionalAmount(team.unattributedMerged, 'merged')),
      // Only a team that merged into two or more repositories has them: for one, they are left out, and stay left out here.
      ...(byRepository.length > 0 ? { byRepository } : {}),
    }
  })
  return { ...header(given), teams }
}

function tokens(raw: unknown): TokenCounts {
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
      ...periodOf(team),
      byType: Object.fromEntries(EFFORT_TYPES.map((type) => [type, { costUsd: amount(asObject(byType[type]).costUsd, `${type} cost`), tokens: tokens(asObject(byType[type]).tokens) }])) as TeamEffort['byType'],
      byDay,
    }
  })
  return { ...header(given), teams }
}
