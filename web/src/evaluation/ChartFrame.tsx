import type { ReactNode } from 'react'
import { useI18n } from '../i18n'

interface ChartFrameProps {
  /** The top of the value axis; the bottom is 0. */
  max: number
  format: (value: number) => string
  /** `YYYY-MM-DD`, oldest first: the first, middle, and last are written under the plot. */
  dates: readonly string[]
  label: string
  children: ReactNode
}

/** The axes and hairline grid every chart of the screen shares: values on the left, days underneath, the marks in between. */
export function ChartFrame({ max, format, dates, label, children }: ChartFrameProps) {
  const { fmt } = useI18n()
  const ticks = [max, max / 2, 0]
  const written = [dates[0], dates[Math.floor((dates.length - 1) / 2)], dates[dates.length - 1]]
  return (
    <div className="ev-chart">
      <div className="ev-axis" aria-hidden="true">
        {ticks.map((tick) => (
          <span key={tick} style={{ bottom: `${(tick / max) * 100}%` }}>{format(tick)}</span>
        ))}
      </div>
      <div className="ev-main">
        <div className="ev-plot" role="group" aria-label={label}>
          {ticks.map((tick) => (
            <i key={tick} className="ev-grid" style={{ bottom: `${(tick / max) * 100}%` }} />
          ))}
          {children}
        </div>
        <div className="ev-dates" aria-hidden="true">
          {written.map((date, index) => (
            <span key={index}>{date === undefined ? '' : fmt.shortDate(date)}</span>
          ))}
        </div>
      </div>
    </div>
  )
}
