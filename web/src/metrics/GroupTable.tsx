import type { LatencySeries } from '../api/metricsApi'
import { useI18n } from '../i18n'
import { groupName } from './groupName'
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
export function GroupTable({ series, kind, title, client }: { series: readonly LatencySeries[]; kind: LatencySeries['kind']; title: string; client?: string }) {
  const { t } = useI18n()
  const text = t.metrics.groups
  // A client's table is that client's series only.
  const groups = groupsOf(client === undefined ? series : series.filter((one) => one.client === client), kind)
  if (groups.length === 0) return null
  // Only what the node answered from its own state is an in-node read, and only in the table of what it answered: a client's observation of
  // the same route has the network in it.
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
                <th scope="row" role="rowheader">{groupName(text.names, group.key)}</th>
                <Cells row={group} />
              </tr>
              {group.routes.map((route) => (
                <tr key={route.key} className="mt-route" role="row">
                  <th scope="row" role="rowheader">
                    <code>{route.key}</code>
                    {kind === 'serve' && inNodeReads.has(route.key) ? <span className="mt-tag">{text.inNodeRead}</span> : null}
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
