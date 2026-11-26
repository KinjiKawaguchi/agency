import type { DayEvaluation } from '../api/evaluationApi'
import { allMerged, countMax, emptyCounts, fillDays, lastDayOf, median, niceMax, rollingCostPerPullRequest, rollingMedian, totalMerged } from './series'

const day = (date: string, overrides: Partial<DayEvaluation> = {}): DayEvaluation => ({ date, merged: emptyCounts(), reverts: 0, mainRedSeconds: 0, costUsd: 0, prLeadSeconds: [], issueLeadSeconds: [], ...overrides })

describe('fillDays', () => {
  it('lists every day of the period, oldest first, with the days the node left out empty', () => {
    const rows = fillDays([day('2026-11-23', { reverts: 1 }), day('2026-11-26', { reverts: 2 })], 5, '2026-11-26')
    expect(rows.map((row) => row.date)).toEqual(['2026-11-22', '2026-11-23', '2026-11-24', '2026-11-25', '2026-11-26'])
    expect(rows.map((row) => row.reverts)).toEqual([0, 1, 0, 0, 2])
  })

  it('crosses a month and a year end', () => {
    expect(fillDays([], 3, '2027-01-01').map((row) => row.date)).toEqual(['2026-12-30', '2026-12-31', '2027-01-01'])
  })
})

describe('lastDayOf', () => {
  it('takes the day the node names', () => {
    expect(lastDayOf([day('2026-11-24')], '2026-11-26', '2026-11-26T00:00:00Z')).toBe('2026-11-26')
  })

  it('takes the newest day the node reports when it does not name one, not the UTC date of the report', () => {
    // 05:00 in Tokyo on the 26th is still the 25th in UTC.
    expect(lastDayOf([day('2026-11-26'), day('2026-11-24')], undefined, '2026-11-25T20:00:00Z')).toBe('2026-11-26')
  })

  it('falls back to the UTC date of the report only when the period has no day at all', () => {
    expect(lastDayOf([], undefined, '2026-11-25T20:00:00Z')).toBe('2026-11-25')
  })

  it('keeps the newest day, and all its numbers, in the days a chart draws, as the node’s summary counts them', () => {
    const byDay = [day('2026-11-25', { reverts: 1 }), day('2026-11-26', { reverts: 2 })]
    const rows = fillDays(byDay, 3, lastDayOf(byDay, undefined, '2026-11-25T20:00:00Z'))
    expect(rows.map((row) => row.date)).toEqual(['2026-11-24', '2026-11-25', '2026-11-26'])
    expect(rows.reduce((sum, row) => sum + row.reverts, 0)).toBe(byDay.reduce((sum, row) => sum + row.reverts, 0))
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
    // Day b merged 2 features and a fix, and day c one revert, which the node counts on its own.
    const rows = [day('a', { costUsd: 10 }), day('b', { costUsd: 20, merged: { ...emptyCounts(), feat: 2, fix: 1 } }), day('c', { costUsd: 30, reverts: 1 })]
    const [first, second, third] = rollingCostPerPullRequest(rows, 2)
    expect(first).toBeNull()
    expect(second).toBe(10)
    expect(third).toBeCloseTo(50 / 4)
    expect(totalMerged(rows[1]!.merged)).toBe(3)
    expect(allMerged(rows[2]!)).toBe(1)
    expect(allMerged({ merged: { ...emptyCounts(), feat: 2 }, reverts: 1 })).toBe(3)
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
