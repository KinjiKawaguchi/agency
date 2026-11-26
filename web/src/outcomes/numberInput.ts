import type { NumberEntry, NumberInput } from '../api/outcomesApi'

export type Problem = 'date' | 'value' | 'note'

export interface NumberForm {
  date: string
  value: string
  note: string
}

const DAY = /^(\d{4})-(\d{2})-(\d{2})$/
const MAX_NOTE = 200
const MAX_VALUE = 1e15

/** A calendar day as written, or nothing when it is not one (2026-02-30 is not). */
function dayOf(text: string): Date | undefined {
  const found = DAY.exec(text)
  if (found === null) return undefined
  const date = new Date(Date.UTC(Number(found[1]), Number(found[2]) - 1, Number(found[3])))
  return date.toISOString().slice(0, 10) === text ? date : undefined
}

const shiftedDay = (date: Date, days: number, years: number): Date => new Date(Date.UTC(date.getUTCFullYear() + years, date.getUTCMonth(), date.getUTCDate() + days))

/**
 * What the owner typed, checked as the node checks it (docs/protocol.md, "Outcomes"), so that a mistake is said before it is
 * sent: a day that is not more than one day ahead of the node's today and not older than ten years, a finite number of at most
 * 1e15, and a note of at most 200 characters without a control character. The node still decides; this only saves the round trip.
 */
export function checkNumber(form: NumberForm, kpi: string, today: string): { ok: true; input: NumberInput } | { ok: false; problem: Problem } {
  const date = dayOf(form.date)
  const now = dayOf(today)
  if (date === undefined || now === undefined || date > shiftedDay(now, 1, 0) || date < shiftedDay(now, 0, -10)) return { ok: false, problem: 'date' }
  const written = form.value.trim()
  const value = written === '' ? Number.NaN : Number(written)
  if (!Number.isFinite(value) || Math.abs(value) > MAX_VALUE) return { ok: false, problem: 'value' }
  const note = form.note.trim()
  // eslint-disable-next-line no-control-regex -- a control character in a note is exactly what is refused
  if ([...note].length > MAX_NOTE || /[\u0000-\u001f\u007f]/.test(note)) return { ok: false, problem: 'note' }
  return { ok: true, input: { kpi, date: form.date, value, ...(note === '' ? {} : { note }) } }
}

/** For each entry of a history (newest first), whether a later one for the same KPI and day has replaced it: the first stays in the history, and the later one wins. */
export function replacedIn(entries: readonly NumberEntry[]): boolean[] {
  const latest = new Map<string, string>()
  for (const entry of entries) {
    const key = `${entry.kpi}/${entry.date}`
    const known = latest.get(key)
    if (known === undefined || entry.enteredAt > known) latest.set(key, entry.enteredAt)
  }
  return entries.map((entry) => entry.enteredAt < (latest.get(`${entry.kpi}/${entry.date}`) ?? entry.enteredAt))
}
