import { DAY_SECONDS, HOUR_SECONDS, niceMax } from './series'

export interface DurationUnits {
  minute: string
  hour: string
  day: string
}

const MINUTE_SECONDS = 60

/** One decimal under ten, none above, and no trailing ".0": 0.5, 1.2, 12. */
const tidy = (value: number): string => (value >= 10 ? String(Math.round(value)) : String(Math.round(value * 10) / 10))

/** A span in the unit that reads best: minutes under an hour, hours under two days, days above. */
export function formatDuration(seconds: number, units: DurationUnits): string {
  if (seconds < HOUR_SECONDS) return `${Math.round(seconds / MINUTE_SECONDS)}${units.minute}`
  if (seconds < 2 * DAY_SECONDS) return `${tidy(seconds / HOUR_SECONDS)}${units.hour}`
  return `${tidy(seconds / DAY_SECONDS)}${units.day}`
}

export interface DurationAxis {
  /** The top of the axis, in seconds. */
  max: number
  format: (seconds: number) => string
}

/** An axis for spans: its top is a round number of one unit (minutes, hours, or days), and every tick is written in that unit. */
export function durationAxis(maxSeconds: number, units: DurationUnits): DurationAxis {
  const unit = maxSeconds < HOUR_SECONDS ? { size: MINUTE_SECONDS, label: units.minute } : maxSeconds <= 2 * DAY_SECONDS ? { size: HOUR_SECONDS, label: units.hour } : { size: DAY_SECONDS, label: units.day }
  const top = niceMax(maxSeconds / unit.size)
  return { max: top * unit.size, format: (seconds) => `${tidy(seconds / unit.size)}${unit.label}` }
}
