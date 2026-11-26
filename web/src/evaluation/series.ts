import type { DayEvaluation, MergedCounts, PullRequestType } from '../api/evaluationApi.ts'

/** The types in the order they stack, and the colors they keep: a filter never repaints another type. */
export const PULL_REQUEST_TYPES: readonly PullRequestType[] = ['feat', 'fix', 'refactor', 'docs', 'test', 'chore', 'other']

export const TYPE_COLORS: Record<PullRequestType, string> = {
  feat: 'var(--series-1)',
  fix: 'var(--series-2)',
  refactor: 'var(--series-3)',
  docs: 'var(--series-4)',
  test: 'var(--series-5)',
  chore: 'var(--series-6)',
  other: 'var(--series-other)',
}

const SECOND = 1
export const HOUR_SECONDS = 3600 * SECOND
export const DAY_SECONDS = 24 * HOUR_SECONDS
const DAY_MS = DAY_SECONDS * 1000

export const emptyCounts = (): MergedCounts => ({ feat: 0, fix: 0, refactor: 0, docs: 0, test: 0, chore: 0, other: 0 })

export const totalMerged = (counts: MergedCounts): number => PULL_REQUEST_TYPES.reduce((sum, type) => sum + counts[type], 0)

/** The days of the period, oldest first and none missing: the node may leave out a day with nothing in it. */
export function fillDays(byDay: readonly DayEvaluation[], days: number, generatedAt: string): DayEvaluation[] {
  const last = Date.parse(`${generatedAt.slice(0, 10)}T00:00:00Z`)
  const known = new Map(byDay.map((day) => [day.date, day]))
  return Array.from({ length: days }, (_, index) => {
    const date = new Date(last - (days - 1 - index) * DAY_MS).toISOString().slice(0, 10)
    return known.get(date) ?? { date, merged: emptyCounts(), reverts: 0, mainRedSeconds: 0, costUsd: 0, prLeadSeconds: [], issueLeadSeconds: [] }
  })
}

export function median(values: readonly number[]): number | null {
  if (values.length === 0) return null
  const sorted = values.toSorted((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? (sorted[middle] ?? null) : ((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2
}

/** Days in the trailing window that ends at each day (the day itself and the ones before it). */
function trailing<T>(items: readonly T[], window: number): T[][] {
  return items.map((_, index) => items.slice(Math.max(0, index - window + 1), index + 1))
}

/** The median of every lead time in the trailing window, per day; `null` while the window has none. */
export function rollingMedian(rows: readonly DayEvaluation[], pick: (row: DayEvaluation) => readonly number[], window: number): (number | null)[] {
  return trailing(rows, window).map((span) => median(span.flatMap(pick)))
}

/** Cost per merged pull request over the trailing window; `null` while nothing was merged in it. */
export function rollingCostPerPullRequest(rows: readonly DayEvaluation[], window: number): (number | null)[] {
  return trailing(rows, window).map((span) => {
    const merged = span.reduce((sum, row) => sum + totalMerged(row.merged), 0)
    return merged === 0 ? null : span.reduce((sum, row) => sum + row.costUsd, 0) / merged
  })
}

/** The smallest of 1, 2, 5 times a power of ten that is at least `value`; 1 for nothing, so an empty chart still has an axis. */
export function niceMax(value: number): number {
  if (!(value > 0)) return 1
  const magnitude = 10 ** Math.floor(Math.log10(value))
  const step = [1, 2, 5, 10].find((candidate) => candidate * magnitude >= value) ?? 10
  return step * magnitude
}


/** An axis maximum for counts: whole numbers all the way, so that the middle tick is whole as well. */
export const countMax = (value: number): number => Math.max(2, 2 * Math.ceil(niceMax(value) / 2))

/** Where a day sits across the plot, from 0 at the first to 1 at the last. */
export const positionOf = (index: number, count: number): number => (count > 1 ? index / (count - 1) : 0.5)
