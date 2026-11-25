import type { Funding, UsageWindow } from '../api/clientApi'
import { useI18n } from '../i18n'
import { forecastWindow } from './forecast'

type WindowName = UsageWindow['name']

/** Weekly first: it is the one that decides how much work fits. */
const WINDOW_ORDER: readonly WindowName[] = ['sevenDay', 'fiveHour']

function WindowBlock({ name, reading, now }: { name: WindowName; reading: UsageWindow | undefined; now: number }) {
  const { t, fmt } = useI18n()
  const label = t.costs.windows[name]
  if (reading === undefined) return <p className="fund-quiet">{t.costs.notStarted(label)}</p>
  const forecast = forecastWindow(reading, now)
  if (forecast === undefined) return <p className="fund-quiet">{t.costs.stale(label)}</p>
  const used = Math.min(100, forecast.utilization)
  const expected = Math.min(100, forecast.projectedAtReset)
  return (
    <div className="fund-window">
      <div className="fund-top">
        <b>{label}</b>
        <span className="fund-pct">{fmt.percent(forecast.utilization)}</span>
        <span className="fund-left">{t.costs.left(fmt.percent(Math.max(0, 100 - forecast.utilization)))}</span>
        <span className={`fund-tag tag-${forecast.status}`}>{t.costs.statuses[forecast.status]}</span>
      </div>
      <div className="fund-bar" role="img" aria-label={t.costs.barLabel(label, fmt.percent(forecast.utilization), fmt.percent(forecast.projectedAtReset))}>
        <span className="used" style={{ width: `${used}%` }} />
        <span className="expected" style={{ left: `${used}%`, width: `${Math.max(0, expected - used)}%` }} />
      </div>
      <p className="fund-line">
        {t.costs.resetLine(fmt.dateTime(forecast.resetsAt), fmt.relative(forecast.resetsAt, t.time.justNow), fmt.percent(forecast.projectedAtReset))}{' '}
        {forecast.hitAt === undefined ? '' : t.costs.hitLine(fmt.dateTime(forecast.hitAt))} {t.costs.verdicts[forecast.status]}
      </p>
    </div>
  )
}

/** One funding source: its weekly and five-hour windows, each projected to its reset. */
export function FundingCard({ source, now }: { source: Funding; now: number }) {
  const { t, fmt } = useI18n()
  return (
    <article className="fund">
      <header className="fund-head">
        <h3>{source.label}</h3>
        <span className="fund-kind">{t.costs.kinds[source.kind]}</span>
      </header>
      {source.windows.length === 0 ? (
        <p className="fund-quiet">{t.costs.unknownUsage}</p>
      ) : (
        WINDOW_ORDER.map((name) => <WindowBlock key={name} name={name} reading={source.windows.find((one) => one.name === name)} now={now} />)
      )}
      {source.readAt === undefined ? null : <p className="fund-teams">{t.costs.readAt(fmt.dateTime(source.readAt))}</p>}
    </article>
  )
}
