import type { Locale } from './locale'

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

export interface Formatters {
  /** "3 hours ago", "in 2 days". Falls back to `justNow` under a minute. */
  relative: (iso: string, justNow: string) => string
  /** "Nov 25, 2:02 PM" */
  dateTime: (iso: string) => string
  /** "2:02 PM" */
  clock: (iso: string) => string
  /** "Tue, Nov 25" */
  day: (iso: string) => string
  /** 12,345 */
  integer: (value: number) => string
}

export function createFormatters(locale: Locale, now: () => number = Date.now): Formatters {
  const relativeFormat = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  const dateTimeFormat = new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })
  const clockFormat = new Intl.DateTimeFormat(locale, { hour: 'numeric', minute: '2-digit' })
  const dayFormat = new Intl.DateTimeFormat(locale, { weekday: 'short', month: 'short', day: 'numeric' })
  const numberFormat = new Intl.NumberFormat(locale)

  return {
    relative: (iso, justNow) => {
      const delta = Date.parse(iso) - now()
      const size = Math.abs(delta)
      if (size < MINUTE) return justNow
      if (size < HOUR) return relativeFormat.format(Math.round(delta / MINUTE), 'minute')
      if (size < 2 * DAY) return relativeFormat.format(Math.round(delta / HOUR), 'hour')
      return relativeFormat.format(Math.round(delta / DAY), 'day')
    },
    dateTime: (iso) => dateTimeFormat.format(new Date(iso)),
    clock: (iso) => clockFormat.format(new Date(iso)),
    day: (iso) => dayFormat.format(new Date(iso)),
    integer: (value) => numberFormat.format(value),
  }
}
