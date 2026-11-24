import type { UsageReport } from '../api/clientApi'
import { columnsFor, MAX_SERIES, seriesFor, totalsOf } from './usage'

const tokens = { input: 10, output: 20, cacheRead: 80, cacheWrite: 10 }

const report: UsageReport = {
  org: 'acme',
  team: 'app',
  days: 2,
  generatedAt: '2026-11-25T00:00:00Z',
  byDay: [
    { date: '2026-11-24', costUsd: { lead: 4, backend: 6 } },
    { date: '2026-11-25', costUsd: { lead: 1, backend: 2, ghost: 3 } },
  ],
  byPerson: [
    { person: 'backend', costUsd: 8, tokens },
    { person: 'lead', costUsd: 5, tokens },
    { person: 'ghost', costUsd: 3, tokens },
  ],
}

const label = (person: string): string => person.toUpperCase()

describe('seriesFor', () => {
  it('follows the team order and gives each person a fixed color', () => {
    const series = seriesFor(report, ['lead', 'backend', 'frontend'], label, 'Other')
    expect(series.map((one) => [one.person, one.color])).toEqual([
      ['lead', 'var(--series-1)'],
      ['backend', 'var(--series-2)'],
      ['ghost', 'var(--series-3)'],
    ])
  })

  it('folds people past the eighth into one other series instead of a ninth hue', () => {
    const many: UsageReport = {
      ...report,
      byPerson: Array.from({ length: MAX_SERIES + 2 }, (_, index) => ({ person: `p${index}`, costUsd: 1, tokens })),
    }
    const series = seriesFor(many, [], label, 'Other')
    expect(series).toHaveLength(MAX_SERIES + 1)
    expect(series.at(-1)).toMatchObject({ person: null, label: 'Other', color: 'var(--series-other)' })
  })
})

describe('columnsFor', () => {
  it('lists the cost of each series for each day, oldest first', () => {
    const series = seriesFor(report, ['lead', 'backend'], label, 'Other')
    expect(columnsFor(report, series).map((column) => [column.date, column.values, column.total])).toEqual([
      ['2026-11-24', [4, 6, 0], 10],
      ['2026-11-25', [1, 2, 3], 6],
    ])
  })

  it('adds the folded people into the other column', () => {
    const series = [
      { person: 'lead', label: 'LEAD', color: 'var(--series-1)' },
      { person: null, label: 'Other', color: 'var(--series-other)' },
    ]
    expect(columnsFor(report, series)[1]?.values).toEqual([1, 5])
  })
})

describe('totalsOf', () => {
  it('adds the people up and reads the cache share from the input side', () => {
    const totals = totalsOf(report)
    expect(totals.costUsd).toBe(16)
    expect(totals.perDay).toBe(8)
    expect(totals.output).toBe(60)
    expect(totals.inputSide).toBe(300)
    expect(totals.cacheShare).toBeCloseTo(0.8)
  })

  it('does not divide by zero when nothing was used', () => {
    expect(totalsOf({ ...report, byDay: [], byPerson: [] })).toMatchObject({ costUsd: 0, perDay: 0, cacheShare: 0 })
  })
})
