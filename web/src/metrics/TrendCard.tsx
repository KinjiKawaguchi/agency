import type { LatencySeries } from '../api/metricsApi'
import { useI18n } from '../i18n'
import { groupName } from './groupName'
import { HourChart } from './HourChart'
import { groupsOf as tableOrderOf } from './latency'
import { hasHours, hoursBetween, isExactTrend, trendOf } from './trend'

interface TrendCardProps {
  series: readonly LatencySeries[]
  from: string
  until: string
  budgetMs: number | undefined
}

const tableOrder = (series: readonly LatencySeries[], kind: LatencySeries['kind']): string[] => tableOrderOf(series, kind).map((row) => row.key)

const groupsOf = (series: readonly LatencySeries[], kind: LatencySeries['kind']): Array<[string, LatencySeries[]]> => {
  const groups = new Map<string, LatencySeries[]>()
  for (const one of series.filter((member) => member.kind === kind)) groups.set(one.group, [...(groups.get(one.group) ?? [] /* stand-in-ok(index): the first of a group has no list yet, an accumulator of the code itself */), one])
  // In the order of the tables above: the busiest group first.
  const order = tableOrder(series, kind)
  return [...groups].sort(([a], [b]) => order.indexOf(a) - order.indexOf(b))
}

/**
 * The p95 hour by hour, one small chart for each route group (and for the in-node reads against the budget), as far as the
 * node gives the hours: for a range of more than 7 days it gives totals only, and the card says so and draws nothing.
 */
export function TrendCard({ series, from, until, budgetMs }: TrendCardProps) {
  const { t } = useI18n()
  const text = t.metrics.trend
  const hours = hoursBetween(from, until)
  if (hours.length === 0) {
    return (
      <div className="card mt-card">
        <h3 className="sub-head">{text.title}</h3>
        <p className="muted">{text.onlyShort}</p>
      </div>
    )
  }
  const reads = series.filter((one) => one.kind === 'serve' && one.isInNodeRead)
  const charts = [
    ...(budgetMs === undefined ? [] : [{ key: 'in-node-reads', title: text.inNodeReads, members: reads, budget: budgetMs }]),
    ...groupsOf(series, 'serve').map(([group, members]) => ({ key: `serve-${group}`, title: groupName(t.metrics.groups.names, group), members, budget: undefined })),
    ...groupsOf(series, 'call').map(([group, members]) => ({ key: `call-${group}`, title: text.callGroup(groupName(t.metrics.groups.names, group)), members, budget: undefined })),
  ].map((chart) => ({ ...chart, points: trendOf(chart.members, hours) })).filter((chart) => hasHours(chart.points))
  if (charts.length === 0) return null
  const isApproximate = charts.some((chart) => !isExactTrend(chart.points))
  return (
    <div className="card mt-card">
      <h3 className="sub-head">{text.title}</h3>
      <p className="muted">{text.lead}</p>
      {isApproximate ? <p className="muted">{text.approximate}</p> : null}
      <div className="mt-trends">
        {charts.map((chart) => (
          <HourChart key={chart.key} title={chart.title} points={chart.points} {...(chart.budget === undefined ? {} : { budgetMs: chart.budget })} />
        ))}
      </div>
    </div>
  )
}
