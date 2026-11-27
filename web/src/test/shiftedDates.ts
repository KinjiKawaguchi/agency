// The moments the weekly run of the tests starts its clock at (scripts/ci/shifted-clock.sh): each of the next seven days
// (every weekday), the end of the month and the start of the next, the end of the year and its start, and, for a few time zones
// that are not UTC, the moment before the next change of their clock. These are the days when something made up from the date
// changes: a weekend, a week, a month, a year, an hour that is 23 or 25 hours long.
export interface ShiftedRun {
  name: string
  /** When the clock of the tests starts, in UTC. */
  now: string
  /** The time zone the tests run in. */
  timeZone: string
}

const HOUR = 3_600_000
const DAY = 24 * HOUR
export const DST_ZONES: readonly string[] = ['America/New_York', 'Europe/London', 'Australia/Sydney']

/** The offset from UTC of a zone at a moment, in minutes, as the zone writes it. */
function offsetMinutes(timeZone: string, at: number): number {
  const name = new Intl.DateTimeFormat('en-US', { timeZone, timeZoneName: 'longOffset' }).formatToParts(new Date(at)).find((part) => part.type === 'timeZoneName')?.value ?? 'GMT'
  const found = /GMT(?:([+-])(\d{2}):(\d{2}))?/.exec(name)
  if (found?.[1] === undefined) return 0
  return (found[1] === '-' ? -1 : 1) * (Number(found[2]) * 60 + Number(found[3]))
}

/** The first moment after `after` at which the zone's clock changes (the hour on the wall jumps), found within `days` days; nothing for a zone that does not change. */
export function nextClockChange(timeZone: string, after: number, days = 400): number | undefined {
  const dayStart = Math.floor(after / DAY) * DAY
  for (let day = 0; day < days; day++) {
    const from = dayStart + day * DAY
    if (offsetMinutes(timeZone, from) === offsetMinutes(timeZone, from + DAY)) continue
    for (let hour = 0; hour < 24; hour++) {
      const at = from + hour * HOUR
      if (at > after && offsetMinutes(timeZone, at - HOUR) !== offsetMinutes(timeZone, at)) return at
    }
  }
  return undefined
}

const iso = (ms: number): string => new Date(ms).toISOString()
const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'] as const

export function shiftedRuns(today: Date, zones: readonly string[] = DST_ZONES): ShiftedRun[] {
  const runs: ShiftedRun[] = []
  const dayStart = Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate())
  for (let ahead = 1; ahead <= 7; ahead++) {
    const noon = dayStart + ahead * DAY + 12 * HOUR
    runs.push({ name: `${WEEKDAYS[new Date(noon).getUTCDay()] ?? 'day'} ${iso(noon).slice(0, 10)}`, now: iso(noon), timeZone: 'UTC' })
  }
  const monthEnd = (month: number): number => Date.UTC(today.getUTCFullYear(), month + 1, 0, 23, 30)
  const endOfMonth = monthEnd(today.getUTCMonth()) > today.getTime() ? monthEnd(today.getUTCMonth()) : monthEnd(today.getUTCMonth() + 1)
  runs.push({ name: 'end of the month', now: iso(endOfMonth), timeZone: 'UTC' }, { name: 'start of the next month', now: iso(endOfMonth + HOUR), timeZone: 'UTC' })
  const yearEnd = Date.UTC(today.getUTCFullYear(), 11, 31, 23, 30) > today.getTime() ? Date.UTC(today.getUTCFullYear(), 11, 31, 23, 30) : Date.UTC(today.getUTCFullYear() + 1, 11, 31, 23, 30)
  runs.push({ name: 'end of the year', now: iso(yearEnd), timeZone: 'UTC' }, { name: 'start of the next year', now: iso(yearEnd + HOUR), timeZone: 'UTC' })
  for (const zone of zones) {
    const change = nextClockChange(zone, today.getTime())
    if (change !== undefined) runs.push({ name: `clock change in ${zone}`, now: iso(change - 10 * 60_000), timeZone: zone })
  }
  return runs
}
