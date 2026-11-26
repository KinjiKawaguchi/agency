import type { DayEffort, EffortType, TeamEffort, TeamPlan, WorkType } from '../api/evaluationApi'

export const WORK_TYPES: readonly WorkType[] = ['newFeature', 'improvement', 'maintenance', 'debt', 'incident']

/** The types in the order they stack, with the work no one could classify last. */
export const EFFORT_TYPES: readonly EffortType[] = [...WORK_TYPES, 'unclassified']

/** Each type keeps its color. New feature has the first color, as `feat` has in the speed charts. */
export const EFFORT_COLORS: Record<EffortType, string> = {
  newFeature: 'var(--series-1)',
  improvement: 'var(--series-2)',
  maintenance: 'var(--series-3)',
  debt: 'var(--series-4)',
  incident: 'var(--series-5)',
  unclassified: 'var(--series-other)',
}

export const WEEK_DAYS = 7

const isCount = (value: unknown): value is number => typeof value === 'number' && Number.isInteger(value) && value >= 0

/**
 * A team's plan from the org definition, or nothing. The definition is the node's and the root key's, so
 * the screen trusts only a plan with the shape the protocol allows: whole numbers that sum to 100, and a
 * whole budget. Anything else is shown as no plan stated, never as a guess.
 */
export function planOf(definition: unknown, team: string): TeamPlan | undefined {
  const teams = (definition as { teams?: unknown } | null)?.teams
  if (!Array.isArray(teams)) return undefined
  const plan = (teams.find((one) => (one as { id?: unknown } | null)?.id === team) as { plan?: unknown } | undefined)?.plan as { allocation?: Record<string, unknown>; weeklyBudgetUsd?: unknown } | undefined
  if (plan === undefined || plan === null || typeof plan.allocation !== 'object' || plan.allocation === null) return undefined
  const shares = WORK_TYPES.map((type) => plan.allocation?.[type])
  if (!shares.every(isCount) || shares.reduce((sum, share) => sum + share, 0) !== 100 || !isCount(plan.weeklyBudgetUsd)) return undefined
  return { allocation: Object.fromEntries(WORK_TYPES.map((type, index) => [type, shares[index]])) as TeamPlan['allocation'], weeklyBudgetUsd: plan.weeklyBudgetUsd }
}

export const totalCost = (effort: TeamEffort): number => EFFORT_TYPES.reduce((sum, type) => sum + effort.byType[type].costUsd, 0)

/** Each type's share of the period's cost, as percentages that add up to 100; all 0 when nothing was spent. */
export function sharesOf(effort: TeamEffort): Record<EffortType, number> {
  const total = totalCost(effort)
  return Object.fromEntries(EFFORT_TYPES.map((type) => [type, total === 0 ? 0 : (effort.byType[type].costUsd / total) * 100])) as Record<EffortType, number>
}

/** What a team spent over the seven days that end at `last` (the period's last day), at API prices. */
export function weekSpend(byDay: readonly DayEffort[], last: string): number {
  const first = Date.parse(`${last}T00:00:00Z`) - (WEEK_DAYS - 1) * 86_400_000
  return byDay.filter((day) => Date.parse(`${day.date}T00:00:00Z`) >= first).reduce((sum, day) => sum + EFFORT_TYPES.reduce((daySum, type) => daySum + day.costUsd[type], 0), 0)
}

/** Percentage points above (+) or below (-) the plan, whole numbers. */
export const pointsFromPlan = (share: number, intended: number): number => Math.round(share) - intended
