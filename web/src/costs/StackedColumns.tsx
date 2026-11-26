import { useI18n } from '../i18n'
import type { Column, Series } from './usage'

interface StackedColumnsProps {
  columns: readonly Column[]
  series: readonly Series[]
}

/**
 * Cost by day, stacked by person. Each column explains itself on hover or keyboard focus, and the
 * per-person table beside it carries the same numbers for anyone who cannot tell the colors apart.
 */
export function StackedColumns({ columns, series }: StackedColumnsProps) {
  const { t, fmt } = useI18n()
  const max = Math.max(...columns.map((column) => column.total), 0.01)
  return (
    <div className="columns-chart">
      <div className="columns-axis" aria-hidden="true">
        <span>{fmt.money(max)}</span>
        <span>$0</span>
      </div>
      <div className="columns" role="group" aria-label={t.costs.tokens.byDay}>
        {columns.map((column) => (
          <div key={column.date} className="column" tabIndex={0} aria-label={t.costs.tokens.dayTotal(fmt.shortDate(column.date), fmt.money(column.total))}>
            <div className="stack" style={{ height: `${(column.total / max) * 100}%` }}>
              {series.map((one, index) => {
                const value = column.values[index] ?? 0 /* stand-in-ok(index): one value for each person in the series, by position */
                return value > 0 ? <span key={one.person ?? 'other'} className="slice" style={{ flex: value, background: one.color }} /> : null
              })}
            </div>
            <div className="tip" aria-hidden="true">
              <b>{fmt.shortDate(column.date)}</b>
              {series.map((one, index) => (
                <span key={one.person ?? 'other'} className="tip-row">
                  <i style={{ background: one.color }} />
                  {one.label}
                  <b>{fmt.money(column.values[index] ?? 0 /* stand-in-ok(index): one value for each person in the series, by position */)}</b>
                </span>
              ))}
              <span className="tip-row total">{fmt.money(column.total)}</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

export function Legend({ series }: { series: readonly Series[] }) {
  if (series.length < 2) return null
  return (
    <ul className="legend">
      {series.map((one) => (
        <li key={one.person ?? 'other'}>
          <i style={{ background: one.color }} />
          {one.label}
        </li>
      ))}
    </ul>
  )
}
