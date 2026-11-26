import { formatDuration, type DurationUnits } from '../evaluation/duration'
import type { KpiInfo } from '../api/outcomesApi'
import type { Locale } from '../i18n/locale'

/**
 * A KPI's number in its own unit, as the node has it: money in its own ISO 4217 currency with that currency's decimals (and
 * never converted), a percent as a number of percent (41.5 is 41.5%, and 120 or -3 are shown as they are, never clipped),
 * a ratio as a multiple (1.5 is 1.5×), a count as a plain number, and seconds as a duration. A unit this client does not
 * know, or a currency code that is not one, is shown as a plain number, with the code beside it.
 */
export function formatKpiValue(value: number, kpi: KpiInfo, locale: Locale, units: DurationUnits): string {
  const plain = (maximumFractionDigits: number): string => new Intl.NumberFormat(locale, { maximumFractionDigits }).format(value)
  switch (kpi.unit) {
    case 'money': {
      if (kpi.currency === undefined) return plain(2)
      try {
        return new Intl.NumberFormat(locale, { style: 'currency', currency: kpi.currency }).format(value)
      } catch {
        return `${plain(2)} ${kpi.currency}`
      }
    }
    case 'percent':
      return `${plain(2)}%`
    case 'ratio':
      return `${plain(2)}×`
    case 'seconds':
      // A duration is read as a span: its sign is kept, its unit chosen by its size.
      return value < 0 ? `−${formatDuration(-value, units)}` : formatDuration(value, units)
    case 'count':
    case 'unknown':
      return plain(2)
  }
}

/** The same for the ticks of an axis, where room is short: money and counts in the short form of the language (¥2M, 1.2K). */
export function formatKpiAxis(value: number, kpi: KpiInfo, locale: Locale, units: DurationUnits): string {
  if (kpi.unit === 'money' && kpi.currency !== undefined) {
    try {
      return new Intl.NumberFormat(locale, { style: 'currency', currency: kpi.currency, notation: 'compact', maximumFractionDigits: 1 }).format(value)
    } catch {
      return formatKpiValue(value, kpi, locale, units)
    }
  }
  if (kpi.unit === 'count' || kpi.unit === 'unknown') return new Intl.NumberFormat(locale, { notation: 'compact', maximumFractionDigits: 1 }).format(value)
  return formatKpiValue(value, kpi, locale, units)
}

/** A change in the KPI's own unit, with its sign: +4 points (of a percent), −3, +1.2×. */
export function formatKpiChange(change: number, kpi: KpiInfo, locale: Locale, units: DurationUnits): string {
  // The change of a percent KPI is a number of points, not a percent of a percent.
  const shown = kpi.unit === 'percent' ? `${new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(Math.abs(change))}${locale === 'ja' ? 'ポイント' : ' points'}` : formatKpiValue(Math.abs(change), kpi, locale, units)
  // A change that is written as 0 has no sign: "+0 points" would say a direction the number does not have.
  const isZero = !/[1-9]/.test(shown.replace(/ポイント|points/, ''))
  return `${isZero ? '' : change > 0 ? '+' : '−'}${shown}`
}
