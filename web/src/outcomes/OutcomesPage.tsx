import { useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { meQuery, outcomesQuery, readingsQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { KpiCard } from './KpiCard'
import { ReadingsSection } from './ReadingsSection'
import { DEFAULT_DAYS, OUTCOMES_PERIODS, useOutcomesAccess } from './useOutcomesAccess'
import './outcomes.css'

interface PageProps {
  org: string | undefined
  days: number | undefined
  onChange: (next: { org: string; days: number }) => void
}

function OutcomesSkeleton() {
  return (
    <Loading>
      <Skeleton kind="title" />
      <div className="oc-grid">
        <Skeleton kind="box" />
        <Skeleton kind="box" />
        <Skeleton kind="box" />
      </div>
    </Loading>
  )
}

/**
 * The owner's numbers: each KPI over the period, and how each aim stands around the day its work shipped. The node answers 403
 * to anyone else, so for them this page shows nothing, and goes to the overview: it is not an error, and not even a hint.
 */
export function OutcomesPage({ org, days, onChange }: PageProps) {
  const { t } = useI18n()
  const api = useApi()
  const access = useOutcomesAccess()
  const navigate = useNavigate()
  const me = useQuery(meQuery(api)).data
  useEffect(() => {
    if (access.status === 'none') void navigate({ to: '/overview', replace: true })
  }, [access.status, navigate])
  if (access.status === 'loading') return <OutcomesSkeleton />
  if (access.status === 'none' || me === undefined) return null
  const chosen = access.orgs.find((one) => one === org) ?? access.orgs[0]
  if (chosen === undefined) return null
  const period = OUTCOMES_PERIODS.find((one) => one === days) ?? DEFAULT_DAYS
  const orgs = me.orgs.filter((one) => access.orgs.includes(one.id))
  const teamLabels = new Map((me.orgs.find((one) => one.id === chosen)?.teams ?? []).map((team) => [team.id, team.label])) /* stand-in-ok(routing): names for the teams of the org chosen, used only where one is */
  return (
    <>
      <h1 className="page-title">{t.outcomes.title}</h1>
      <p className="muted ev-lead">{t.outcomes.lead}</p>
      <div className="ev-controls">
        {orgs.length > 1 ? (
          <label className="ev-org">
            <span className="muted">{t.outcomes.org}</span>
            <select value={chosen} onChange={(event) => onChange({ org: event.target.value, days: period })}>
              {orgs.map((one) => (
                <option key={one.id} value={one.id}>{one.label}</option>
              ))}
            </select>
          </label>
        ) : null}
        <div className="period" role="group" aria-label={t.outcomes.period}>
          {OUTCOMES_PERIODS.map((one) => (
            <button key={one} className={`btn small${one === period ? ' primary' : ''}`} type="button" aria-pressed={one === period} onClick={() => onChange({ org: chosen, days: one })}>
              {t.outcomes.days(one)}
            </button>
          ))}
        </div>
      </div>
      <p className="muted">{t.outcomes.note}</p>
      <OrgOutcomes org={chosen} days={period} teamLabels={teamLabels} />
    </>
  )
}

function OrgOutcomes({ org, days, teamLabels }: { org: string; days: number; teamLabels: ReadonlyMap<string, string> }) {
  const { t } = useI18n()
  const api = useApi()
  const numbers = useQuery(outcomesQuery(api, org, days))
  const readings = useQuery(readingsQuery(api, org))
  if (numbers.data === undefined) {
    return numbers.error === null ? <Loading quiet><div className="oc-grid"><Skeleton kind="box" /><Skeleton kind="box" /></div></Loading> : <p className="empty" role="alert">{failureText(t, numbers.error)}</p>
  }
  const { series, until } = numbers.data
  return (
    <>
      <section aria-labelledby="oc-measures">
        <h2 className="sub-head" id="oc-measures">{t.outcomes.measures}</h2>
        {series.length === 0 ? <p className="empty">{t.outcomes.noMeasures}</p> : null}
        <div className="oc-grid">
          {series.map((one) => (
            <KpiCard key={one.kpi.id} series={one} until={until} days={numbers.data.days} />
          ))}
        </div>
      </section>
      {readings.data === undefined ? (
        readings.error === null ? <Loading quiet><Skeleton kind="box" /></Loading> : <p className="empty" role="alert">{failureText(t, readings.error)}</p>
      ) : (
        <ReadingsSection readings={readings.data} kpis={series.map((one) => one.kpi)} teamLabels={teamLabels} />
      )}
    </>
  )
}
