import type { DayEvaluation } from '../api/evaluationApi'
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
    expect(byDay.every((day) => day.prLeadSeconds.length === Object.values(day.merged).reduce((sum, count) => sum + count, 0) + day.reverts)).toBe(true)
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

describe('the mock’s summary, against numbers worked out by hand', () => {
  const merged = { feat: 1, fix: 0, refactor: 0, docs: 0, test: 0, chore: 0, other: 0 }
  const day = (date: string, prLeadSeconds: number[], overrides: Partial<DayEvaluation> = {}): DayEvaluation => ({ date, merged, reverts: 0, mainRedSeconds: 0, costUsd: 0, prLeadSeconds, issueLeadSeconds: [], ...overrides })

  it('takes the median of all samples and the nearest-rank p90', () => {
    // Four lead times of 1, 2, 3, and 4 hours: the median is 2.5 hours; the p90 is the 4th of 4 (ceil(0.9 * 4)).
    const summary = summaryOf([day('2026-11-24', [3600, 10_800]), day('2026-11-25', [7200, 14_400])])
    expect(summary.prLeadMedianSeconds).toBe(9000)
    expect(summary.prLeadP90Seconds).toBe(14_400)
    expect(summary.issueLeadMedianSeconds).toBeNull()
  })

  it('takes the p90 of ten samples as the ninth', () => {
    const tenHours = Array.from({ length: 10 }, (_, index) => (index + 1) * 3600)
    expect(summaryOf([day('2026-11-25', tenHours)]).prLeadP90Seconds).toBe(9 * 3600)
  })

  it('adds up the red time, finds the longest day, and divides the cost by every merged pull request, the reverts included', () => {
    // Two typed pull requests and one revert: $15 over 3.
    const summary = summaryOf([day('2026-11-24', [], { mainRedSeconds: 600, costUsd: 10, reverts: 1 }), day('2026-11-25', [], { mainRedSeconds: 1800, costUsd: 5 })])
    expect(summary.mainRedSeconds).toBe(2400)
    expect(summary.longestRedSeconds).toBe(1800)
    expect(summary.reverts).toBe(1)
    expect(summary.costPerMergedPrUsd).toBe(5)
  })
})
