import { daysEnding, niceMax } from '../evaluation/series'
import { TrendChart } from '../evaluation/TrendChart'
import type { KpiSeries } from '../api/outcomesApi'
import { useI18n } from '../i18n'
import { EnterNumber } from './EnterNumber'
import { NumberHistory } from './NumberHistory'
import { formatKpiAxis, formatKpiChange, formatKpiValue, isWrittenAsZero } from './format'
import { movementOf } from './movement'

interface KpiCardProps {
  org: string
  series: KpiSeries
  /** The period the numbers are drawn over: its last day, and how many days. */
  until: string
  days: number
}

/** A chart cannot draw a number below its floor; a KPI that has one is listed as numbers, not drawn. */
const isDrawable = (values: readonly number[]): boolean => values.every((value) => value >= 0)

/** One KPI: its latest number, how it moved from the baseline, and its numbers over the period with a gap on each day that has none. */
export function KpiCard({ org, series, until, days }: KpiCardProps) {
  const { t, fmt, locale } = useI18n()
  const text = t.outcomes
  const { kpi, points, before } = series
  const latest = points.at(-1)
  const show = (value: number): string => formatKpiValue(value, kpi, locale, t.evaluation.units)
  const units = t.evaluation.units
  const movement = latest !== undefined && before !== undefined ? movementOf(before.value, latest.value, kpi.goodWhen, isWrittenAsZero(latest.value - before.value, kpi, locale, units)) : undefined
  const dates = daysEnding(until, days)
  const byDate = new Map(points.map((point) => [point.date, point.value]))
  const values = dates.map((date) => byDate.get(date) ?? null)
  const drawable = points.length > 0 && isDrawable(points.map((point) => point.value))
  return (
    <article className="oc-kpi card" aria-label={kpi.label}>
      <h3 className="sub-head">{kpi.label}</h3>
      {kpi.goodWhen === undefined ? null : <p className="muted oc-good">{kpi.goodWhen === 'higher' ? text.higher : text.lower}</p>}
      {latest === undefined ? (
        <>
          <p className="muted oc-none">{before === undefined ? text.noNumbersAny : text.noNumbers}</p>
          {before === undefined ? null : <p className="muted">{text.lastNumber(show(before.value), fmt.shortDate(before.date))}</p>}
        </>
      ) : (
        <>
          <p className="oc-value">{show(latest.value)}</p>
          <p className="muted">{text.lastNumberIn(fmt.shortDate(latest.date))}</p>
        </>
      )}
      {movement === undefined || latest === undefined || before === undefined ? null : (
        <p className={`oc-move${movement.quality === undefined ? '' : ` is-${movement.quality}`}`}>
          {movement.direction === 'unchanged'
            ? text.movement.same(show(before.value), fmt.shortDate(before.date))
            : text.movement.from(text.movement[movement.direction], formatKpiChange(latest.value - before.value, kpi, locale, t.evaluation.units), show(before.value), fmt.shortDate(before.date))}
          {movement.quality === undefined ? null : <b className="oc-quality">{text.movement[movement.quality]}</b>}
        </p>
      )}
      {points.length === 0 ? null : drawable ? (
        <TrendChart
          dates={dates}
          lines={[{ key: kpi.id, label: kpi.label, color: 'var(--series-1)', values }]}
          max={niceMax(Math.max(...points.map((point) => point.value)))}
          format={show}
          tickFormat={(value) => formatKpiAxis(value, kpi, locale, t.evaluation.units)}
          label={text.trend(kpi.label)}
          noData={text.chartNoData}
        />
      ) : (
        <ul className="oc-list" aria-label={text.listed}>
          {points.map((point) => (
            <li key={point.date}>
              <span className="muted">{fmt.shortDate(point.date)}</span> {show(point.value)}
            </li>
          ))}
        </ul>
      )}
      <div className="oc-actions">
        <EnterNumber org={org} kpi={kpi} today={until} />
        <NumberHistory org={org} kpi={kpi} />
      </div>
    </article>
  )
}
