import type { Evaluation } from '../api/evaluationApi'
import { costByDay } from './costByDay'

const day = (date: string, costUsd: number) => ({ date, costUsd }) as unknown as Evaluation['teams'][number]['byDay'][number]
const team = (name: string, days: Array<[string, number]>) => ({ team: name, byDay: days.map(([date, usd]) => day(date, usd)) }) as unknown as Evaluation['teams'][number]
const evaluation = (teams: Evaluation['teams'], unavailable?: Evaluation['unavailable']): Evaluation => ({ org: 'acme', days: 3, generatedAt: '2026-11-24T12:00:00Z', teams, ...(unavailable === undefined ? {} : { unavailable }) }) as unknown as Evaluation
const DATES = ['2026-11-22', '2026-11-23', '2026-11-24']

describe('the cost on the days a KPI is drawn over', () => {
  it('is the sum of what each team reports for the day', () => {
    const { days } = costByDay(evaluation([team('app', [['2026-11-22', 1.5], ['2026-11-23', 2], ['2026-11-24', 0]]), team('site', [['2026-11-22', 0.5], ['2026-11-23', 3], ['2026-11-24', 4]])]), DATES)
    expect(days.map((one) => one.usd)).toEqual([2, 5, 4])
  })

  it('keeps a day of 0 that a team reported as 0, and has no cost, not 0, for a day no team reported', () => {
    const { days } = costByDay(evaluation([team('app', [['2026-11-22', 0], ['2026-11-24', 3]])]), DATES)
    expect(days.map((one) => one.usd)).toEqual([0, undefined, 3])
  })

  it('has no cost at all when there is no team', () => {
    expect(costByDay(evaluation([]), DATES).days.every((one) => one.usd === undefined)).toBe(true)
  })

  it('counts the teams that could not report, so the sum can say it is of the others', () => {
    expect(costByDay(evaluation([team('app', [['2026-11-22', 1]])], [{ team: 'site', reason: 'down' }] as unknown as Evaluation['unavailable']), DATES).teamsWithout).toBe(1)
    expect(costByDay(evaluation([team('app', [['2026-11-22', 1]])]), DATES).teamsWithout).toBe(0)
  })
})
