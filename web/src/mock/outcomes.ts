import { readNumberEntry } from '../api/outcomesAnswers'
import type { NumberEntry } from '../api/outcomesApi'
import { sampleOutcomes } from '../api/outcomesSamples'
import { daysEnding } from '../evaluation/series'

interface RawSeries {
  kpi: { id: string }
  before?: unknown
  points: unknown[]
}
interface RawOutcomes {
  org: string
  until: string
  generatedAt: string
  series: RawSeries[]
}

/** What the node says of a write it refuses: a status and the reason. */
export interface Refusal {
  status: number
  message: string
}

const isRefusal = (value: unknown): value is Refusal => typeof value === 'object' && value !== null && 'status' in value

/**
 * The owner's numbers as the mock node keeps them: the shared sample's, then whatever is entered. The latest entry
 * for a KPI and a day wins in the period's answer, and every entry stays in the history, as the real node does.
 */
export function createMockOutcomes(startedAt: Date) {
  const sample = sampleOutcomes as RawOutcomes
  const kpis = sample.series.map((one) => one.kpi.id)
  let entries: readonly NumberEntry[] = sample.series.flatMap((one) => [...(one.before === undefined ? [] : [one.before]), ...one.points].map(readNumberEntry))
  let entered = 0

  const effective = (kpi: string): NumberEntry[] => {
    const byDay = new Map<string, NumberEntry>()
    for (const entry of entries.filter((one) => one.kpi === kpi)) {
      const known = byDay.get(entry.date)
      if (known === undefined || Date.parse(entry.enteredAt) >= Date.parse(known.enteredAt)) byDay.set(entry.date, entry)
    }
    return [...byDay.values()].toSorted((a, b) => a.date.localeCompare(b.date))
  }

  return {
    answer(days: number): unknown {
      const from = daysEnding(sample.until, days)[0] ?? sample.until
      return {
        org: sample.org,
        days,
        from,
        until: sample.until,
        generatedAt: sample.generatedAt,
        series: sample.series.map((raw) => {
          const all = effective(raw.kpi.id)
          const before = all.filter((one) => one.date < from).at(-1)
          return { ...raw, points: all.filter((one) => one.date >= from && one.date <= sample.until), ...(before === undefined ? {} : { before }) }
        }),
      }
    },
    history(kpi: string, limit: number): NumberEntry[] | Refusal {
      if (!kpis.includes(kpi)) return { status: 404, message: 'No such measure.' }
      return entries.filter((one) => one.kpi === kpi).toSorted((a, b) => Date.parse(b.enteredAt) - Date.parse(a.enteredAt)).slice(0, limit)
    },
    enter(body: unknown): NumberEntry | Refusal {
      const given = (typeof body === 'object' && body !== null ? body : {}) as Record<string, unknown>
      if (typeof given.kpi !== 'string' || !kpis.includes(given.kpi)) return { status: 404, message: 'No such measure.' }
      if (typeof given.date !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(given.date)) return { status: 400, message: 'The day is not a day.' }
      if (typeof given.value !== 'number' || !Number.isFinite(given.value)) return { status: 400, message: 'The value is not a number.' }
      if (given.note !== undefined && (typeof given.note !== 'string' || [...given.note].length > 200)) return { status: 400, message: 'The note is at most 200 characters.' }
      entered += 1
      const entry: NumberEntry = {
        kpi: given.kpi,
        date: given.date,
        value: given.value,
        source: { kind: 'manual' },
        enteredAt: new Date(startedAt.getTime() + entered * 1000).toISOString(),
        enteredBy: 'owner',
        ...(typeof given.note === 'string' && given.note !== '' ? { note: given.note } : {}),
      }
      entries = [...entries, entry]
      return entry
    },
    isRefusal,
  }
}
