import type { Evaluation } from '../api/evaluationApi'
import { costByDay } from './costByDay'

type Team = Evaluation['teams'][number]
const team = (name: string, period: [string, string], days: Array<[string, number]>): Team => ({ team: name, from: period[0], until: period[1], byDay: days.map(([date, costUsd]) => ({ date, costUsd })) }) as unknown as Team
const evaluation = (teams: Team[], unavailable: Evaluation['unavailable'] = []): Evaluation => ({ org: 'acme', days: 3, generatedAt: '2026-11-24T12:00:00Z', teams, unavailable }) as Evaluation
const DATES = ['2026-11-22', '2026-11-23', '2026-11-24']
const ALL: [string, string] = ['2026-11-22', '2026-11-24']

describe('the cost on the days a KPI is drawn over', () => {
  it('is the sum of what each team reports for the day', () => {
    const { days } = costByDay(evaluation([team('app', ALL, [['2026-11-22', 1.5], ['2026-11-23', 2], ['2026-11-24', 0]]), team('site', ALL, [['2026-11-22', 0.5], ['2026-11-23', 3], ['2026-11-24', 4]])]), DATES)
    expect(days.map((one) => one.usd)).toEqual([2, 5, 4])
  })

  it('is 0 for a day inside a team’s period that its byDay leaves out, as the node leaves out a day with nothing in it', () => {
    const { days } = costByDay(evaluation([team('app', ALL, [['2026-11-24', 3]])]), DATES)
    expect(days.map((one) => one.usd)).toEqual([0, 0, 3])
  })

  it('has no cost for a day no team’s period covers, which is not a day of 0', () => {
    const { days } = costByDay(evaluation([team('app', ['2026-11-23', '2026-11-24'], [['2026-11-23', 2]])]), DATES)
    expect(days.map((one) => one.usd)).toEqual([undefined, 2, 0])
  })

  it('counts a day for the teams whose period covers it, and does not add 0 for one that does not', () => {
    const { days } = costByDay(evaluation([team('app', ALL, [['2026-11-22', 1]]), team('late', ['2026-11-24', '2026-11-24'], [['2026-11-24', 5]])]), DATES)
    expect(days.map((one) => one.usd)).toEqual([1, 0, 5])
  })

  it('has no cost at all when there is no team', () => {
    expect(costByDay(evaluation([]), DATES).days.every((one) => one.usd === undefined)).toBe(true)
  })

  it('counts the teams that could not report, so the sum can say it is of the others', () => {
    expect(costByDay(evaluation([team('app', ALL, [])], [{ team: 'site', reason: 'down' }] as Evaluation['unavailable']), DATES).teamsWithout).toBe(1)
    expect(costByDay(evaluation([team('app', ALL, [])]), DATES).teamsWithout).toBe(0)
  })
})
