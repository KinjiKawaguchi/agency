import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import type { LatencySeries, Metrics } from '../api/metricsApi'
import { metricsQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Skeleton } from '../shared/Skeleton'
import { BudgetCard } from './BudgetCard'
import { GroupTable } from './GroupTable'
import { Objectives } from './Objectives'
import { TrendCard } from './TrendCard'
import './metrics.css'

export const METRICS_PERIODS = [1, 7, 30] as const

/**
 * The node's own response times, for the owner. Nothing of the section is drawn until the node has answered for the
 * first time (only a skeleton): a node that says this is not the viewer's to read (403) then leaves nothing behind, with
 * no heading and no hint that there is something; a node that does not measure (404) says so in one line; an answer
 * with no series says nothing was measured, which is not 0 ms. Another period keeps the section and draws a skeleton
 * where the numbers were, since numbers of the first period must not stand under the second one's button.
 */
export function NodeSpeedSection() {
  const { t } = useI18n()
  const api = useApi()
  const [days, setDays] = useState<(typeof METRICS_PERIODS)[number]>(1)
  const metrics = useQuery({ ...metricsQuery(api, days), placeholderData: keepPreviousData })
  const failure = metrics.error instanceof ApiError ? metrics.error : null
  // The owner's own section is not there for anyone else: not an error, and nothing to say.
  if (failure?.status === 403) return null
  if (metrics.data === undefined && metrics.error === null) return <Skeleton kind="box" />
  const text = t.metrics
  return (
    <section className="ev-team mt-section" aria-labelledby="mt-title">
      <h2 className="sub-head" id="mt-title">{text.title}</h2>
      <p className="muted">{text.lead}</p>
      <div className="period" role="group" aria-label={text.period}>
        {METRICS_PERIODS.map((one) => (
          <button key={one} className={`btn small${one === days ? ' primary' : ''}`} type="button" aria-pressed={one === days} onClick={() => setDays(one)}>
            {text.days(one)}
          </button>
        ))}
      </div>
      {metrics.data === undefined ? <p className="empty" role={failure?.status === 404 ? undefined : 'alert'}>{failure?.status === 404 ? text.unavailable : failure?.status === 503 ? text.couldNotAnswer : failureText(t, metrics.error as Error)}</p> : metrics.isPlaceholderData ? <Skeleton kind="box" /> : <Report metrics={metrics.data} />}
    </section>
  )
}

/** What each client that reports waited for its own calls, apart: the web client and the apps are not one reader, and a network in between is in every figure. */
function ClientTables({ series }: { series: readonly LatencySeries[] }) {
  const { t } = useI18n()
  const text = t.metrics.groups
  const clients = [...new Set(series.filter((one) => one.kind === 'client').map((one) => one.client))].sort((a, b) => clientOrder(a) - clientOrder(b))
  return (
    <>
      {clients.map((client) => (
        <GroupTable key={client ?? 'none'} series={series} client={client} kind="client" title={text.clientTitle(client === 'web' ? text.clients.web : client === 'ios' ? text.clients.ios : text.clients.other)} />
      ))}
    </>
  )
}

const clientOrder = (client: string | undefined): number => (client === 'web' ? 0 : client === 'ios' ? 1 : 2)

function Report({ metrics }: { metrics: Metrics }) {
  const { t } = useI18n()
  if (metrics.series.length === 0) return <p className="empty">{t.metrics.none}</p>
  return (
    <>
      {metrics.budgetInNodeReadP95Ms === undefined ? null : <BudgetCard series={metrics.series} budgetMs={metrics.budgetInNodeReadP95Ms} />}
      <Objectives objectives={metrics.objectives} />
      <GroupTable series={metrics.series} kind="serve" title={t.metrics.groups.title} />
      <GroupTable series={metrics.series} kind="call" title={t.metrics.groups.callsTitle} />
      <ClientTables series={metrics.series} />
      <TrendCard series={metrics.series} from={metrics.from /* shown-ok(period): the first time of the range of the series */} until={metrics.until} budgetMs={metrics.budgetInNodeReadP95Ms} />
    </>
  )
}
