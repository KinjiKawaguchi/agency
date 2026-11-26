import type { Evaluation } from '../api/evaluationApi'

export interface CostDay {
  date: string
  /** The API-priced cost of the day over the teams that reported it; a day no team reported is not a day of 0. */
  usd: number | undefined
}

/**
 * The org's cost on each of the days a KPI is drawn over: the sum of what each team reports for the day. A day that
 * no team's answer covers has no cost (`undefined`), which is not a cost of 0, and a team that did not answer at all
 * (`unavailable`) is counted so that the screen can say the sum is of the others.
 */
export function costByDay(evaluation: Evaluation, dates: readonly string[]): { days: CostDay[]; teamsWithout: number } {
  const perDate = new Map<string, number>()
  for (const team of evaluation.teams) {
    for (const day of team.byDay) perDate.set(day.date, (perDate.get(day.date) ?? 0) + day.costUsd) /* stand-in-ok(index): a sum started when the day is first seen */
  }
  const unavailable = evaluation.unavailable
  return { days: dates.map((date) => ({ date, usd: perDate.get(date) })), teamsWithout: unavailable === undefined ? 0 : unavailable.length }
}
