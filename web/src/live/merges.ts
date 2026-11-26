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

/** The median of the given values, rounded down to a whole number; nothing when there are none. */
export function medianOf(values: readonly number[]): number | undefined {
  if (values.length === 0) return undefined
  const sorted = values.toSorted((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 1 ? sorted[middle] : Math.floor(((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2)
}
