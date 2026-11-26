import { useI18n } from '../i18n'
import type { HourPoint, HourValue } from './trend'
import { useBoundText } from './useBound'

/** The scale is in milliseconds only, drawn on a log scale from the first bound to the last (a p95 runs from 1 ms to 30 s). */
const SCALE_MIN_MS = 1
const SCALE_MAX_MS = 30_000
const TICKS_MS = [1, 10, 100, 1000, 10_000]

const share = (ms: number): number => (Math.log10(Math.min(Math.max(ms, SCALE_MIN_MS), SCALE_MAX_MS) / SCALE_MIN_MS) / Math.log10(SCALE_MAX_MS / SCALE_MIN_MS)) * 100

/** Where a value stands, as a share of the plot's height; what is slower than the last bound is at the top. */
const heightOf = (value: HourValue): number | undefined => (value.kind === 'none' ? undefined : value.kind === 'beyond' ? 100 : share(value.ms))

const centerOf = (index: number, count: number): number => ((index + 0.5) / count) * 100

/** A line that stops where an hour had no call and starts again after it: a gap is not a 0. */
function pathOf(points: readonly HourPoint[]): string {
  let isBroken = true
  return points
    .map((point, index) => {
      const height = heightOf(point.value)
      if (height === undefined) {
        isBroken = true
        return ''
      }
      const command = isBroken ? 'M' : 'L'
      isBroken = false
      return `${command}${centerOf(index, points.length).toFixed(2)} ${(100 - height).toFixed(2)}`
    })
    .join('')
}

interface HourChartProps {
  title: string
  points: readonly HourPoint[]
  /** The budget, drawn as a horizontal line; only on the chart of the reads it applies to. */
  budgetMs?: number
}

/**
 * One group's p95 for each hour: the slowest route's, at most. One axis, in milliseconds. A gap is an hour with no call.
 * For anyone who cannot see the picture the same hours are a table under it.
 */
export function HourChart({ title, points, budgetMs }: HourChartProps) {
  const { t, fmt } = useI18n()
  const text = t.metrics.trend
  const { duration, bound } = useBoundText()
  const written = (at: number): string => fmt.dateTime(new Date(at).toISOString())
  const withCalls = points.filter((point) => point.value.kind !== 'none')
  const boundOf = (value: HourValue): string => (value.kind === 'none' ? '' : bound(value.kind === 'beyond' ? { kind: 'beyond' } : { kind: 'atMost', ms: value.ms }))
  const first = points[0]
  const last = points[points.length - 1]
  const lastWithCalls = withCalls[withCalls.length - 1]
  return (
    <figure className="mt-trend">
      <figcaption className="mt-trend-title">{title}</figcaption>
      <div className="mt-chart">
        <div className="mt-axis" aria-hidden="true">
          {TICKS_MS.map((tick) => (
            <span key={tick} style={{ bottom: `${share(tick)}%` }}>{duration(tick)}</span>
          ))}
        </div>
        <div className="mt-main">
          <div className="mt-plot" role="img" aria-label={lastWithCalls === undefined ? title : text.summary(title, String(withCalls.length), boundOf(lastWithCalls.value))}>
            {TICKS_MS.map((tick) => (
              <i key={tick} className="mt-grid" style={{ bottom: `${share(tick)}%` }} />
            ))}
            {budgetMs === undefined ? null : (
              <i className="mt-budget" style={{ bottom: `${share(budgetMs)}%` }}>
                <b>{text.budget(duration(budgetMs))}</b>
              </i>
            )}
            <svg className="mt-line" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
              <path d={pathOf(points)} />
            </svg>
            {points.map((point, index) => {
              const height = heightOf(point.value)
              return height === undefined ? null : <i key={point.at} className={`mt-dot${point.value.kind === 'beyond' ? ' is-beyond' : ''}`} style={{ left: `${centerOf(index, points.length)}%`, bottom: `${height}%` }} />
            })}
          </div>
          <div className="mt-times" aria-hidden="true">
            <span>{first === undefined ? '' : written(first.at)}</span>
            <span>{last === undefined ? '' : written(last.at)}</span>
          </div>
        </div>
      </div>
      <details className="ev-details">
        <summary>{text.show}</summary>
        <div className="ev-table-wrap">
          <table>
            <caption className="sr-only">{text.caption(title)}</caption>
            <thead>
              <tr>
                <th scope="col">{text.hour}</th>
                <th scope="col" className="num">{text.calls}</th>
                <th scope="col" className="num">p95</th>
              </tr>
            </thead>
            <tbody>
              {withCalls.toReversed().map((point) => (
                <tr key={point.at}>
                  <th scope="row">{written(point.at)}</th>
                  <td className="num">{fmt.integer(point.count)}</td>
                  <td className="num">{boundOf(point.value)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </figure>
  )
}
