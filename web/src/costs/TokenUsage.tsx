import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { usageQuery } from '../api/queries'
import type { TeamView } from '../api/teamview'
import { useI18n } from '../i18n'
import { Legend, StackedColumns } from './StackedColumns'
import { columnsFor, seriesFor, totalsOf } from './usage'

function Tile({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <div className="tile">
      <span className="tile-label">{label}</span>
      <span className="tile-value">{value}</span>
      <span className="tile-note">{note}</span>
    </div>
  )
}

/** Token use of one team: totals, cost by day, and a table by person. */
export function TokenUsage({ view, days }: { view: TeamView; days: number }) {
  const { t, fmt } = useI18n()
  const api = useApi()
  const usage = useQuery(usageQuery(api, view.org, view.team, days))
  if (usage.data === undefined) {
    return <p className={usage.error === null ? 'muted' : 'empty'} role={usage.error === null ? undefined : 'alert'}>{usage.error === null ? t.costs.tokens.loading : t.app.unreachable(usage.error.message)}</p>
  }
  const report = usage.data
  const labelOf = (person: string): string => view.people.find((one) => one.id === person)?.label ?? person
  const series = seriesFor(report, view.people.map((one) => one.id), labelOf, t.costs.tokens.other)
  const totals = totalsOf(report)
  const { columns: text } = t.costs.tokens
  return (
    <>
      <div className="tiles">
        <Tile label={t.costs.tokens.total(days)} value={fmt.money(totals.costUsd)} note={t.costs.tokens.average(fmt.money(totals.perDay))} />
        <Tile label={t.costs.tokens.output} value={fmt.compact(totals.output)} note={t.costs.tokens.inputSide(fmt.compact(totals.inputSide))} />
        <Tile label={t.costs.tokens.cacheShare} value={fmt.percent(totals.cacheShare * 100)} note={t.costs.tokens.cacheNote} />
      </div>
      <div className="card">
        <p className="card-head">
          <b>{t.costs.tokens.byDay}</b> <span className="muted">{t.costs.tokens.byDayHint}</span>
        </p>
        <Legend series={series} />
        <StackedColumns columns={columnsFor(report, series)} series={series} />
      </div>
      <div className="card flush table-wrap">
        <table>
          <caption className="sr-only">{t.costs.tokens.byPerson}</caption>
          <thead>
            <tr>
              <th scope="col">{text.person}</th>
              <th scope="col" className="num">{text.cost}</th>
              <th scope="col" className="num">{text.output}</th>
              <th scope="col" className="num">{text.cacheRead}</th>
              <th scope="col" className="num">{text.inputWrite}</th>
            </tr>
          </thead>
          <tbody>
            {report.byPerson.map((entry) => (
              <tr key={entry.person}>
                <th scope="row">{labelOf(entry.person)}</th>
                <td className="num">{fmt.money(entry.costUsd)}</td>
                <td className="num">{fmt.compact(entry.tokens.output)}</td>
                <td className="num">{fmt.compact(entry.tokens.cacheRead)}</td>
                <td className="num">{fmt.compact(entry.tokens.input + entry.tokens.cacheWrite)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  )
}
