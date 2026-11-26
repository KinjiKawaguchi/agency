import type { TokenCounts, TokenUsage } from '../api/clientApi'

/** Categorical colors available for series. A ninth person is folded into "other", never given a new hue. */
export const MAX_SERIES = 8

export interface Series {
  /** The team runtime's person name, or `null` for the folded "other" series. */
  person: string | null
  label: string
  /** A CSS variable, fixed per person so a filter never repaints the survivors. */
  color: string
}

export interface Column {
  date: string
  total: number
  /** Cost per series, in series order. */
  values: number[]
}

/**
 * Series follow the team's own person order, so a person keeps their color as the period changes.
 * People beyond the eighth, and any person the team no longer lists, are folded into one "other" series.
 */
export function seriesFor(report: TokenUsage, teamOrder: readonly string[], labelOf: (person: string) => string, otherLabel: string): Series[] {
  const used = new Set(report.byPerson.map((entry) => entry.person))
  const ordered = [...teamOrder.filter((person) => used.has(person)), ...[...used].filter((person) => !teamOrder.includes(person))]
  const shown = ordered.slice(0, MAX_SERIES)
  const series: Series[] = shown.map((person, index) => ({ person, label: labelOf(person), color: `var(--series-${index + 1})` }))
  return ordered.length > MAX_SERIES ? [...series, { person: null, label: otherLabel, color: 'var(--series-other)' }] : series
}

export function columnsFor(report: TokenUsage, series: readonly Series[]): Column[] {
  const named = new Set(series.flatMap((one) => (one.person === null ? [] : [one.person])))
  return report.byDay.map((day) => {
    const values = series.map((one) =>
      one.person === null
        ? Object.entries(day.costUsd).filter(([person]) => !named.has(person)).reduce((sum, [, cost]) => sum + cost, 0)
        : (day.costUsd[one.person] ?? 0 /* stand-in-ok(sparse): a person who cost nothing on a day is left out of that day by the node */),
    )
    return { date: day.date, total: values.reduce((sum, value) => sum + value, 0), values }
  })
}

export interface Totals {
  costUsd: number
  perDay: number
  output: number
  inputSide: number
  /** Share of the input side that was read from the cache, 0 to 1. */
  cacheShare: number
}

const sumTokens = (items: readonly TokenCounts[]): TokenCounts =>
  items.reduce((sum, tokens) => ({ input: sum.input + tokens.input, output: sum.output + tokens.output, cacheRead: sum.cacheRead + tokens.cacheRead, cacheWrite: sum.cacheWrite + tokens.cacheWrite }), { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 })

export function totalsOf(report: TokenUsage): Totals {
  const tokens = sumTokens(report.byPerson.map((entry) => entry.tokens))
  const inputSide = tokens.input + tokens.cacheRead + tokens.cacheWrite
  const costUsd = report.byPerson.reduce((sum, entry) => sum + entry.costUsd, 0)
  return {
    costUsd,
    perDay: costUsd / Math.max(report.byDay.length, 1),
    output: tokens.output,
    inputSide,
    cacheShare: inputSide === 0 ? 0 : tokens.cacheRead / inputSide,
  }
}
