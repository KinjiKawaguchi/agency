import { useQuery } from '@tanstack/react-query'
import { ApiError } from '../api/client'
import type { TokenUsage } from '../api/clientApi'
import { useApi } from '../api/context'
import { usageQuery } from '../api/queries'
import type { Team } from '../api/teams'
import type { TeamView } from '../api/teamview'
import { failureText } from '../api/failure'
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

const PERIODS = [7, 14, 30] as const

/** Totals, cost by day, and a table by person, from one usage report. */
function TokenReport({ view, days, report }: { view: TeamView; days: number; report: TokenUsage }) {
  const { t, fmt } = useI18n()
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

interface TokenSectionProps {
  team: Team
  days: number
  onDaysChange: (days: number) => void
}

/**
 * One team's token use under its heading. 404 means the team's runtime does not report it, and the
 * section then says so instead of showing numbers. 503 means the team's node or runtime could not
 * answer just now.
 */
export function TokenSection({ team, days, onDaysChange }: TokenSectionProps) {
  const { t } = useI18n()
  const api = useApi()
  const { view } = team
  const usage = useQuery(usageQuery(api, view.org, view.team, days))
  const failure = usage.error instanceof ApiError ? usage.error : null
  const isUnavailable = failure?.status === 404
  const couldNotAnswer = failure?.status === 503
  return (
    <section>
      <h2 className="sub-head">{t.costs.tokens.title(view.label)}</h2>
      {isUnavailable ? (
        <p className="empty">{t.costs.tokens.unavailable}</p>
      ) : (
        <>
          <div className="period" role="group" aria-label={t.costs.tokens.period}>
            {PERIODS.map((period) => (
              <button key={period} className={`btn small${period === days ? ' primary' : ''}`} type="button" aria-pressed={period === days} onClick={() => onDaysChange(period)}>
                {t.costs.tokens.days(period)}
              </button>
            ))}
            <span className="muted">{t.costs.tokens.estimate}</span>
          </div>
          {usage.data !== undefined ? (
            <TokenReport view={view} days={days} report={usage.data} />
          ) : (
            <p className={usage.error === null ? 'muted' : 'empty'} role={usage.error === null ? undefined : 'alert'}>
              {usage.error === null ? t.costs.tokens.loading : couldNotAnswer ? t.costs.tokens.couldNotAnswer(usage.error.message) : failureText(t, usage.error)}
            </p>
          )}
        </>
      )}
    </section>
  )
}
