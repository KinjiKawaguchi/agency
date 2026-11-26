import type { LatencySeries } from '../api/metricsApi'
import { useI18n } from '../i18n'
import { groupsOf, type Bound, type Row } from './latency'
import { useBoundText } from './useBound'

function Cells({ row }: { row: Row }) {
  const { t, fmt } = useI18n()
  const labels = t.metrics.groups
  const { bound } = useBoundText()
  // No durations (a row of calls that were all left out) has no percentile: a dash would stand for a number, so the cell says nothing.
  const cell = (value: Bound | undefined): string => (value === undefined ? '' : bound(value))
  return (
    <>
      <td className="num" data-label={labels.requests} role="cell">{fmt.integer(row.count)}</td>
      <td className="num" data-label={labels.errors} role="cell">{fmt.integer(row.errors)}</td>
      <td className="num" data-label="p50" role="cell">{cell(row.p50)}</td>
      <td className="num" data-label="p95" role="cell">{cell(row.p95)}</td>
      <td className="num" data-label="p99" role="cell">{cell(row.p99)}</td>
    </>
  )
}

/**
 * The series of one kind by route group: each group's percentiles come from its buckets put together, and each group
 * opens to its routes. Nothing is drawn for a kind with no measured call.
 */
export function GroupTable({ series, kind, title }: { series: readonly LatencySeries[]; kind: LatencySeries['kind']; title: string }) {
  const { t } = useI18n()
  const text = t.metrics.groups
  const groups = groupsOf(series, kind)
  if (groups.length === 0) return null
  const inNodeReads = new Set(series.filter((one) => one.isInNodeRead).map((one) => one.name))
  return (
    <div className="card mt-card">
      <h3 className="sub-head">{title}</h3>
      <div className="mt-scroll">
        <table className="mt-table" role="table">
          <thead role="rowgroup">
            <tr role="row">
              <th scope="col" role="columnheader">{text.group}</th>
              <th scope="col" className="num" role="columnheader">{text.requests}</th>
              <th scope="col" className="num" role="columnheader">{text.errors}</th>
              <th scope="col" className="num" role="columnheader">p50</th>
              <th scope="col" className="num" role="columnheader">p95</th>
              <th scope="col" className="num" role="columnheader">p99</th>
            </tr>
          </thead>
          {groups.map((group) => (
            <tbody key={group.key} role="rowgroup">
              <tr className="mt-group" role="row">
                <th scope="row" role="rowheader">{text.names[group.key] ?? group.key}</th>
                <Cells row={group} />
              </tr>
              {group.routes.map((route) => (
                <tr key={route.key} className="mt-route" role="row">
                  <th scope="row" role="rowheader">
                    <code>{route.key}</code>
                    {inNodeReads.has(route.key) ? <span className="mt-tag">{text.inNodeRead}</span> : null}
                  </th>
                  <Cells row={route} />
                </tr>
              ))}
            </tbody>
          ))}
        </table>
      </div>
    </div>
  )
}
