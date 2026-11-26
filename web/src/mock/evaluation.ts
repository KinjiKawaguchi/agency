// Made-up evaluation numbers for the mock node: a team that ships most days, with a quiet weekend, a
// couple of red spells on main, and a revert now and then. The shape is the one the real node will
// serve; the numbers are invented. Summaries are worked out from the days, as the node does.
import type { DayEvaluation, Evaluation, EvaluationSummary, MergedCounts, TeamEvaluation } from '../api/evaluationApi.ts'

const TYPES = ['feat', 'fix', 'refactor', 'docs', 'test', 'chore', 'other'] as const

const emptyCounts = (): MergedCounts => ({ feat: 0, fix: 0, refactor: 0, docs: 0, test: 0, chore: 0, other: 0 })
const totalMerged = (counts: MergedCounts): number => TYPES.reduce((sum, type) => sum + counts[type], 0)

function median(values: readonly number[]): number | null {
  if (values.length === 0) return null
  const sorted = values.toSorted((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  // The mean of the two middle samples, rounded down to a whole second, as the node does.
  return sorted.length % 2 === 1 ? (sorted[middle] ?? null) : Math.floor(((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2)
}

const DAY_MS = 86_400_000
const HOUR = 3600

/** A repeatable number in [0, 1) for a seed, so a reload shows the same chart. */
function noise(seed: string): number {
  let hash = 2166136261
  for (const character of seed) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619)
  return ((hash >>> 0) % 10_000) / 10_000
}

/** How many pull requests of each type a team merges on an average working day. */
const RATES: Record<string, MergedCounts> = {
  app: { feat: 1.6, fix: 1.0, refactor: 0.6, docs: 0.4, test: 0.3, chore: 0.5, other: 0.05 },
  site: { feat: 0.8, fix: 0.7, refactor: 0.2, docs: 0.5, test: 0.1, chore: 0.3, other: 0.05 },
}

/** Whole pull requests with the given average: the fraction decides, by noise, whether one more is merged. */
const countFor = (rate: number, seed: string): number => Math.floor(rate) + (noise(seed) < rate - Math.floor(rate) ? 1 : 0) + (noise(`${seed}:burst`) > 0.93 ? 1 : 0)

/** Seconds of a lead time: a few hours most of the time, a long tail of days. */
const leadSeconds = (seed: string, scale: number): number => Math.round(scale * HOUR * (0.25 + 2.2 * noise(seed) ** 2.4 * 3) * (noise(`${seed}:tail`) > 0.92 ? 5 : 1))

function dayFor(team: string, date: string): DayEvaluation {
  const weekday = new Date(`${date}T00:00:00Z`).getUTCDay()
  const isWeekend = weekday === 0 || weekday === 6
  const rates = RATES[team] ?? RATES.site ?? emptyCounts()
  const merged = Object.fromEntries(TYPES.map((type) => [type, isWeekend ? (noise(`${team}:${date}:${type}:w`) < 0.12 ? 1 : 0) : countFor(rates[type], `${team}:${date}:${type}`)])) as MergedCounts
  const count = totalMerged(merged)
  // A revert is a merged pull request counted on its own, not in a type.
  const reverts = count > 0 && noise(`${team}:${date}:revert`) > 0.96 ? 1 : 0
  const prLeadSeconds = Array.from({ length: count + reverts }, (_, index) => leadSeconds(`${team}:${date}:pr:${index}`, 2.2))
  // A pull request opened from an issue that was started earlier; most have one.
  const issueLeadSeconds = prLeadSeconds.filter((_, index) => noise(`${team}:${date}:has-issue:${index}`) < 0.8).map((seconds, index) => seconds + Math.round((6 + 40 * noise(`${team}:${date}:before:${index}`)) * HOUR))
  const isRedDay = noise(`${team}:${date}:red`) > 0.8
  return {
    date,
    merged,
    reverts,
    mainRedSeconds: isRedDay ? Math.round((0.3 + 5 * noise(`${team}:${date}:red-for`)) * HOUR) : 0,
    costUsd: Math.round((isWeekend ? 4 : 16) * (0.5 + 1.4 * noise(`${team}:${date}:cost`)) * 100) / 100,
    prLeadSeconds,
    issueLeadSeconds,
  }
}

/** Nearest rank: the smallest value that at least `share` of the samples do not exceed. */
const percentile = (values: readonly number[], share: number): number | null => {
  if (values.length === 0) return null
  const sorted = values.toSorted((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.ceil(share * sorted.length) - 1)] ?? null
}

/** A field the node leaves out when it has nothing to say (an optional field is never null). */
const left = <K extends string>(key: K, value: number | null): { [P in K]?: number } => (value === null ? {} : ({ [key]: value } as { [P in K]?: number }))

export function summaryOf(days: readonly DayEvaluation[]): EvaluationSummary {
  const merged = emptyCounts()
  for (const day of days) for (const type of TYPES) merged[type] += day.merged[type]
  const prLead = days.flatMap((day) => day.prLeadSeconds)
  const issueLead = days.flatMap((day) => day.issueLeadSeconds)
  const cost = days.reduce((sum, day) => sum + day.costUsd, 0)
  const count = totalMerged(merged) + days.reduce((sum, day) => sum + day.reverts, 0)
  return {
    merged,
    ...left('prLeadMedianSeconds', median(prLead)),
    ...left('prLeadP90Seconds', percentile(prLead, 0.9)),
    ...left('issueLeadMedianSeconds', median(issueLead)),
    ...left('issueLeadP90Seconds', percentile(issueLead, 0.9)),
    mainRedSeconds: days.reduce((sum, day) => sum + day.mainRedSeconds, 0),
    longestRedSeconds: Math.max(0, ...days.map((day) => day.mainRedSeconds)),
    reverts: days.reduce((sum, day) => sum + day.reverts, 0),
    ...left('costPerMergedPrUsd', count === 0 ? null : Math.round((cost / count) * 100) / 100),
  }
}

export function teamEvaluation(team: string, days: number, now: Date): TeamEvaluation {
  const dates = Array.from({ length: days }, (_, index) => new Date(now.getTime() - (days - 1 - index) * DAY_MS).toISOString().slice(0, 10))
  // A day with nothing in it is left out, as the node may do.
  const byDay = dates.map((date) => dayFor(team, date)).filter((day) => totalMerged(day.merged) + day.reverts > 0 || day.mainRedSeconds > 0 || day.costUsd > 0)
  return { team, from: dates[0] ?? '', until: dates.at(-1) ?? '', summary: summaryOf(byDay), byDay }
}

/** What the team's owner reads besides the team's numbers: how each participant spent the period, and what it cost. */
function withParticipants(numbers: TeamEvaluation, days: number): TeamEvaluation {
  const merged = Object.values(numbers.summary.merged).reduce((sum, count) => sum + count, 0) + numbers.summary.reverts
  const share = (part: number): number => Math.round((merged * part) / 10)
  const [backend, lead] = [share(5), share(3)]
  const hours = (count: number): number => count * 3600 * Math.max(1, days / 14)
  const person = (name: string, busy: number, waiting: number, idle: number, cost: number, count: number) => ({
    name,
    busySeconds: hours(busy),
    waitingSeconds: hours(waiting),
    idleSeconds: hours(idle),
    costUsd: cost,
    tokens: { input: Math.round(cost * 14000), output: Math.round(cost * 5300), cacheRead: Math.round(cost * 280000), cacheWrite: Math.round(cost * 21000) },
    merged: count,
    ...(count === 0 ? {} : { costPerMergedPrUsd: Math.round((cost / count) * 100) / 100 }),
  })
  return { ...numbers, participants: [person('backend', 8.5, 0.7, 1.3, 8.45 * Math.max(1, days / 14), backend), person('lead', 5, 2.4, 3.1, 4.1 * Math.max(1, days / 14), lead), person('reviewer', 2, 0.5, 6, 1.6 * Math.max(1, days / 14), 0)], unattributedMerged: Math.max(0, merged - backend - lead) }
}

export function evaluationReport(org: string, teams: readonly string[], days: number, now: Date, isOwner = false): Evaluation {
  const date = (daysAgo: number): string => new Date(now.getTime() - daysAgo * DAY_MS).toISOString().slice(0, 10)
  return { org, days, generatedAt: now.toISOString(), from: date(days - 1), until: date(0), teams: teams.map((team) => (isOwner ? withParticipants(teamEvaluation(team, days, now), days) : teamEvaluation(team, days, now))), unavailable: [] }
}
