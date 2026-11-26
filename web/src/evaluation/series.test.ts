import type { DayEvaluation } from '../api/evaluationApi'
import { countMax, emptyCounts, fillDays, median, niceMax, rollingCostPerPullRequest, rollingMedian, totalMerged } from './series'

const day = (date: string, overrides: Partial<DayEvaluation> = {}): DayEvaluation => ({ date, merged: emptyCounts(), reverts: 0, mainRedSeconds: 0, costUsd: 0, prLeadSeconds: [], issueLeadSeconds: [], ...overrides })

describe('fillDays', () => {
  it('lists every day of the period, oldest first, with the days the node left out empty', () => {
    const rows = fillDays([day('2026-11-23', { reverts: 1 }), day('2026-11-26', { reverts: 2 })], 5, '2026-11-26T09:30:00Z')
    expect(rows.map((row) => row.date)).toEqual(['2026-11-22', '2026-11-23', '2026-11-24', '2026-11-25', '2026-11-26'])
    expect(rows.map((row) => row.reverts)).toEqual([0, 1, 0, 0, 2])
  })

  it('crosses a month and a year end', () => {
    expect(fillDays([], 3, '2027-01-01T00:00:00Z').map((row) => row.date)).toEqual(['2026-12-30', '2026-12-31', '2027-01-01'])
  })
})

describe('median', () => {
  it('is the middle value, the mean of the two middle ones, or nothing', () => {
    expect(median([5, 1, 3])).toBe(3)
    expect(median([4, 1, 3, 2])).toBe(2.5)
    expect(median([])).toBeNull()
  })
})

describe('rollingMedian', () => {
  const rows = [day('a', { prLeadSeconds: [10] }), day('b'), day('c', { prLeadSeconds: [30, 50] }), day('d'), day('e'), day('f', { prLeadSeconds: [90] })]

  it('pools every lead time of the days in the trailing window', () => {
    expect(rollingMedian(rows, (row) => row.prLeadSeconds, 3)).toEqual([10, 10, 30, 40, 40, 90])
  })

  it('says nothing while the window holds no lead time', () => {
    expect(rollingMedian([day('a'), day('b')], (row) => row.prLeadSeconds, 2)).toEqual([null, null])
  })
})

describe('rollingCostPerPullRequest', () => {
  it('divides the window’s cost by the window’s merged pull requests, and says nothing when none merged', () => {
    const rows = [day('a', { costUsd: 10 }), day('b', { costUsd: 20, merged: { ...emptyCounts(), feat: 2, fix: 1 } }), day('c', { costUsd: 30 })]
    const [first, second, third] = rollingCostPerPullRequest(rows, 2)
    expect(first).toBeNull()
    expect(second).toBe(10)
    expect(third).toBeCloseTo(50 / 3)
    expect(totalMerged(rows[1]!.merged)).toBe(3)
  })
})

describe('axis tops', () => {
  it('rounds up to 1, 2, or 5 times a power of ten', () => {
    expect([0, 0.8, 1.2, 3, 7, 120, 5000].map(niceMax)).toEqual([1, 1, 2, 5, 10, 200, 5000])
  })

  it('keeps counts whole in the middle of the axis too', () => {
    expect([0, 1, 2, 3, 5, 7, 11].map(countMax)).toEqual([2, 2, 2, 6, 6, 10, 20])
  })
})
