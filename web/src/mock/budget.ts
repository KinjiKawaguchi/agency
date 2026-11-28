// The mock node's weekly budget: what `GET /api/orgs/{org}/teams/{team}/budget` answers on a node that has the `budget` capability. The
// states are the node's own rules, written out for fixed figures (warning from 80% of the cap, over at the cap, an org that is over makes
// its teams over), so that a screen is drawn against each of them.
import type { BudgetStatus } from '../api/clientApi.ts'

/** The Monday of the week that contains `now`, in UTC, as `YYYY-MM-DD`. */
export function mondayOf(now: Date): string {
  const day = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()))
  day.setUTCDate(day.getUTCDate() - ((day.getUTCDay() + 6) % 7))
  return day.toISOString().slice(0, 10)
}

/**
 * What a team's budget is by default, by the node's rules for the mock's plan (only `app` has one, of $100 a week): the org's cap and spend
 * are those of the teams that have a plan, so the org is as close to its cap as `app` is; a team with no plan has no cap and is `ok`.
 */
export function budgetOf(team: string, now: Date): BudgetStatus {
  const weekStart = mondayOf(now)
  const org = { capUsd: 100, spentUsd: 86.4, state: 'warning' } as const
  if (team === 'app') return { weekStart, team: { capUsd: 100, spentUsd: 86.4, state: 'warning' }, org }
  return { weekStart, team: { spentUsd: 147.8, state: 'ok' }, org }
}

/** The states a screenshot of the meter is made of, by name: what the node would answer for the `app` team (the only one with a plan). */
const STATES: Readonly<Record<string, BudgetStatus>> = {
  ok: { weekStart: '2026-11-23', team: { capUsd: 100, spentUsd: 41.5, state: 'ok' }, org: { capUsd: 100, spentUsd: 41.5, state: 'ok' } },
  over: { weekStart: '2026-11-23', team: { capUsd: 100, spentUsd: 127.9, state: 'over' }, org: { capUsd: 100, spentUsd: 127.9, state: 'over' } },
  partial: { weekStart: '2026-11-23', team: { capUsd: 100, spentUsd: 0, state: 'ok', partial: true }, org: { capUsd: 100, spentUsd: 0, state: 'ok', partial: true } },
}

/** The `budget` option of the mock node: its default figures, or the named state for the `app` team (`ok`, `over`, `partial`). */
export function budgetFor(name: string | null | undefined): true | Readonly<Record<string, BudgetStatus>> {
  const state = name === undefined || name === null ? undefined : STATES[name]
  return state === undefined ? true : { app: state }
}
