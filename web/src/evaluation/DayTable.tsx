import type { DayEvaluation } from '../api/evaluationApi'
import { useI18n } from '../i18n'
import { formatDuration } from './duration'
import { allMerged } from './series'

/** The same days as the charts, in figures: for anyone who cannot tell the colors apart, and to copy from. */
export function DayTable({ team, rows }: { team: string; rows: readonly DayEvaluation[] }) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.table
  return (
    <details className="ev-details">
      <summary>{text.show}</summary>
      <div className="ev-table-wrap">
        <table>
          <caption className="sr-only">{text.caption(team)}</caption>
          <thead>
            <tr>
              <th scope="col">{text.date}</th>
              <th scope="col" className="num">{text.merged}</th>
              <th scope="col" className="num">{text.red}</th>
              <th scope="col" className="num">{text.reverts}</th>
              <th scope="col" className="num">{text.cost}</th>
            </tr>
          </thead>
          <tbody>
            {rows.toReversed().map((row) => (
              <tr key={row.date}>
                <th scope="row">{fmt.shortDate(row.date)}</th>
                <td className="num">{fmt.integer(allMerged(row))}</td>
                <td className="num">{formatDuration(row.mainRedSeconds, t.evaluation.units)}</td>
                <td className="num">{fmt.integer(row.reverts)}</td>
                <td className="num">{fmt.money(row.costUsd)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </details>
  )
}
