import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { metricsQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Skeleton } from '../shared/Skeleton'
import { BudgetCard } from './BudgetCard'
import { GroupTable } from './GroupTable'
import { Objectives } from './Objectives'
import './metrics.css'

export const METRICS_PERIODS = [1, 7, 30] as const

/**
 * The node's own response times, for the owner. Nothing is drawn until the node answers; a node that says this is not
 * the viewer's to read (403) draws nothing at all, with no hint that there is something; a node that does not measure
 * (404) says so in one line; an answer with no series says nothing was measured, which is not 0 ms.
 */
export function NodeSpeedSection() {
  const { t } = useI18n()
  const api = useApi()
  const [days, setDays] = useState<(typeof METRICS_PERIODS)[number]>(1)
  const metrics = useQuery(metricsQuery(api, days))
  const failure = metrics.error instanceof ApiError ? metrics.error : null
  // The owner's own section is not there for anyone else: not an error, and nothing to say.
  if (failure?.status === 403) return null
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
      {metrics.data === undefined ? (
        metrics.error === null ? <Skeleton kind="box" /> : failure?.status === 404 ? <p className="empty">{text.unavailable}</p> : <p className="empty" role="alert">{failure?.status === 503 ? text.couldNotAnswer(metrics.error.message) : failureText(t, metrics.error)}</p>
      ) : metrics.data.series.length === 0 ? (
        <p className="empty">{text.none}</p>
      ) : (
        <>
          {metrics.data.budgetInNodeReadP95Ms === undefined ? null : <BudgetCard series={metrics.data.series} budgetMs={metrics.data.budgetInNodeReadP95Ms} />}
          <Objectives objectives={metrics.data.objectives} />
          <GroupTable series={metrics.data.series} kind="serve" title={text.groups.title} />
          <GroupTable series={metrics.data.series} kind="call" title={text.groups.callsTitle} />
        </>
      )}
    </section>
  )
}
