// Made-up effort numbers for the mock node: what each team spent, by work type. A team's daily total is the
// cost the evaluation route reports for the same day, so the two screens agree. Every name is fictional.
import type { DayEffort, Effort, EffortType, TeamEffort, TeamPlan } from '../api/evaluationApi.ts'
import { teamEvaluation } from './evaluation.ts'

const EFFORT_TYPES = ['newFeature', 'improvement', 'maintenance', 'debt', 'incident', 'unclassified'] as const

/** The mix of work each team runs at, before the noise of a day. */
const MIX: Record<string, Record<EffortType, number>> = {
  app: { newFeature: 0.44, improvement: 0.17, maintenance: 0.2, debt: 0.09, incident: 0.04, unclassified: 0.06 },
  site: { newFeature: 0.2, improvement: 0.24, maintenance: 0.4, debt: 0.06, incident: 0.02, unclassified: 0.08 },
}

/** What the org intends for each team. The `site` team has no plan stated yet. */
export const MOCK_PLANS: Record<string, TeamPlan> = {
  app: { allocation: { newFeature: 50, improvement: 20, maintenance: 20, debt: 10, incident: 0 }, weeklyBudgetUsd: 100 },
}

/** The org definition's team entries with the plans of the mock, as the signed definition will carry them. */
export const withPlans = <T extends { id: string }>(teams: readonly T[]): Array<T & { plan?: TeamPlan }> =>
  teams.map((team) => (MOCK_PLANS[team.id] === undefined ? team : { ...team, plan: MOCK_PLANS[team.id] }))

function noise(seed: string): number {
  let hash = 2166136261
  for (const character of seed) hash = Math.imul(hash ^ character.charCodeAt(0), 16777619)
  return ((hash >>> 0) % 10_000) / 10_000
}

const cents = (value: number): number => Math.round(value * 100) / 100

function dayEffort(team: string, date: string, costUsd: number): DayEffort {
  const mix: Partial<Record<EffortType, number>> = MIX[team] ?? MIX.site ?? {}
  const weights = EFFORT_TYPES.map((type) => (mix[type] ?? 0) * (0.5 + noise(`${team}:${date}:${type}`)))
  const sum = weights.reduce((total, weight) => total + weight, 0)
  return { date, costUsd: Object.fromEntries(EFFORT_TYPES.map((type, index) => [type, cents((costUsd * (weights[index] ?? 0)) / sum)])) as Record<EffortType, number> }
}

export function teamEffort(team: string, days: number, now: Date): TeamEffort {
  const byDay = teamEvaluation(team, days, now).byDay.map((day) => dayEffort(team, day.date, day.costUsd))
  const byType = Object.fromEntries(
    EFFORT_TYPES.map((type) => {
      const costUsd = cents(byDay.reduce((sum, day) => sum + day.costUsd[type], 0))
      return [type, { costUsd, tokens: { input: Math.round(costUsd * 9_000), output: Math.round(costUsd * 24_000), cacheRead: Math.round(costUsd * 610_000), cacheWrite: Math.round(costUsd * 38_000) } }]
    }),
  ) as TeamEffort['byType']
  return { team, byType, byDay }
}

export function effortReport(org: string, teams: readonly string[], days: number, now: Date): Effort {
  const date = (daysAgo: number): string => new Date(now.getTime() - daysAgo * 86_400_000).toISOString().slice(0, 10)
  return { org, days, generatedAt: now.toISOString(), from: date(days - 1), until: date(0), teams: teams.map((team) => teamEffort(team, days, now)) }
}
