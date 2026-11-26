import type { Evaluation } from '../api/evaluationApi'

export interface CostDay {
  date: string
  /** The API-priced cost of the day over the teams whose period covers it; `undefined` when no team's period does. */
  usd: number | undefined
}

/**
 * The org's cost on each of the days a KPI is drawn over: the sum of what each team reports for the day. A team reports a
 * day when it lies inside the team's own period (`from` to `until`, in its own node's time zone): the node leaves out of
 * `byDay` a day with nothing in it, so a day inside the period that `byDay` omits is a day of 0. A day inside no team's
 * period has no cost (`undefined`), which is not a cost of 0. A team that did not answer at all (`unavailable`) is counted
 * so that the screen can say the sum is of the others.
 */
export function costByDay(evaluation: Evaluation, dates: readonly string[]): { days: CostDay[]; teamsWithout: number } {
  const days = dates.map((date): CostDay => {
    const covering = evaluation.teams.filter((team) => team.from <= date && date <= team.until)
    if (covering.length === 0) return { date, usd: undefined }
    return { date, usd: covering.reduce((sum, team) => sum + (team.byDay.find((one) => one.date === date)?.costUsd ?? 0), 0) } /* stand-in-ok(sparse): the node leaves out a day of the team's period that has nothing in it: 0 is what it says */
  })
  return { days, teamsWithout: evaluation.unavailable.length }
}
