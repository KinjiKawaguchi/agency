import { totalMerged } from '../evaluation/series'
import { evaluationReport, summaryOf, teamEvaluation } from './evaluation'

const NOW = new Date('2026-11-26T09:00:00Z')

describe('the mock evaluation', () => {
  it('draws the same numbers every time', () => {
    expect(teamEvaluation('app', 30, NOW)).toEqual(teamEvaluation('app', 30, NOW))
  })

  it('works its summary out from its days, as the node does', () => {
    const { byDay, summary } = teamEvaluation('app', 30, NOW)
    expect(summary).toEqual(summaryOf(byDay))
    expect(summary.merged.feat).toBe(byDay.reduce((sum, day) => sum + day.merged.feat, 0))
    expect(summary.prLeadMedianSeconds).not.toBeNull()
    expect(byDay.every((day) => day.prLeadSeconds.length === totalMerged(day.merged))).toBe(true)
  })

  it('keeps the period: dates end today, ascend, and number at most `days`', () => {
    const { byDay } = teamEvaluation('site', 14, NOW)
    expect(byDay.length).toBeLessThanOrEqual(14)
    expect(byDay.at(-1)?.date).toBe('2026-11-26')
    expect(byDay.map((day) => day.date)).toEqual(byDay.map((day) => day.date).toSorted())
  })

  it('has one entry for each team named', () => {
    expect(evaluationReport('acme', ['app', 'site'], 14, NOW).teams.map((team) => team.team)).toEqual(['app', 'site'])
  })
})
