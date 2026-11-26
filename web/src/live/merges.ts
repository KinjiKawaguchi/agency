import type { TaggedEvent } from './stream'

export const BIN_MS = 5 * 60_000
export const BINS = 12

const mergedAt = (events: readonly TaggedEvent[]): number[] => events.filter(({ event }) => event.kind === 'prMerged').map(({ event }) => Date.parse(event.at))

/** The pull requests merged in each 5 minutes of the last hour: the last bin ends at `now`, the first is the oldest. */
export function binsOf(events: readonly TaggedEvent[], now: number): number[] {
  const bins = Array.from({ length: BINS }, () => 0)
  for (const at of mergedAt(events)) {
    const age = now - at
    if (age < 0 || age >= BINS * BIN_MS) continue
    const index = BINS - 1 - Math.floor(age / BIN_MS)
    bins[index] = (bins[index] ?? 0) + 1
  }
  return bins
}

/** The pull requests merged in the last hour. */
export const lastHour = (events: readonly TaggedEvent[], now: number): number => binsOf(events, now).reduce((sum, count) => sum + count, 0)

/** The start of the viewer's day containing `now`, in ms. */
export function startOfDay(now: number): number {
  const day = new Date(now)
  day.setHours(0, 0, 0, 0)
  return day.getTime()
}

export interface Counts {
  lastHour: number
  today: number
}

/**
 * The two counts the page shows, from the same events, so that they cannot contradict each other: the day's merges
 * are at least the merges of the last hour that fall in the day, and at least what the evaluation counted for the
 * day (which covers the part of the day before the feed's first read). An hour that reaches back over midnight has
 * merges that are not today's: only then is the hour's count above the day's, and rightly.
 */
export function countsOf(events: readonly TaggedEvent[], now: number, evaluationToday: number | undefined): Counts {
  const midnight = startOfDay(now)
  const merged = mergedAt(events).filter((at) => at <= now)
  const hour = merged.filter((at) => now - at < BINS * BIN_MS)
  const sinceMidnight = merged.filter((at) => at >= midnight).length
  return { lastHour: hour.length, today: Math.max(evaluationToday ?? 0, sinceMidnight) }
}

/** The median of the given values, rounded down to a whole number; nothing when there are none. */
export function medianOf(values: readonly number[]): number | undefined {
  if (values.length === 0) return undefined
  const sorted = values.toSorted((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? sorted[middle] : Math.floor(((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2)
}
