import { useQuery } from '@tanstack/react-query'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { evaluationQuery, meQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { NodeSpeedSection } from '../metrics/NodeSpeedSection'
import { EffortSection } from './EffortSection'
import { TeamEvaluation } from './TeamEvaluation'
import './evaluation.css'

export const PERIODS = [14, 30, 90] as const
const DEFAULT_DAYS = 14

interface PageProps {
  /** The org to evaluate; the first of the viewer's orgs when absent or unknown. */
  org: string | undefined
  days: number | undefined
  onChange: (next: { org?: string; days?: number }) => void
}

/** How fast each team of an org ships, from the node's evaluation route. */
export function EvaluationPage({ org, days, onChange }: PageProps) {
  const { t } = useI18n()
  const api = useApi()
  const { data: me, error } = useQuery(meQuery(api))
  if (me === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : failureText(t, error)}</p>
  }
  const chosen = me.orgs.find((one) => one.id === org) ?? me.orgs[0]
  const period = PERIODS.find((one) => one === days) ?? DEFAULT_DAYS
  const teamLabels = new Map((chosen?.teams ?? [] /* stand-in-ok(routing): names for the teams of the org chosen; used only where an org is chosen */).map((team) => [team.id, team.label]))
  return (
    <>
      <h1 className="page-title">{t.evaluation.title}</h1>
      <p className="muted ev-lead">{t.evaluation.lead}</p>
      {chosen === undefined ? (
        <p className="empty">{t.evaluation.noOrgs}</p>
      ) : (
        <>
          <div className="ev-controls">
            {me.orgs.length > 1 ? (
              <label className="ev-org">
                <span className="muted">{t.evaluation.org}</span>
                <select value={chosen.id} onChange={(event) => onChange({ org: event.target.value, days: period })}>
                  {me.orgs.map((one) => (
                    <option key={one.id} value={one.id}>{one.label}</option>
                  ))}
                </select>
              </label>
            ) : null}
            <div className="period" role="group" aria-label={t.evaluation.period}>
              {PERIODS.map((one) => (
                <button key={one} className={`btn small${one === period ? ' primary' : ''}`} type="button" aria-pressed={one === period} onClick={() => onChange({ org: chosen.id, days: one })}>
                  {t.evaluation.days(one)}
                </button>
              ))}
            </div>
          </div>
          <OrgEvaluation org={chosen.id} days={period} teamLabels={teamLabels} />
          <EffortSection org={chosen.id} days={period} teamLabels={teamLabels} />
        </>
      )}
      <NodeSpeedSection />
    </>
  )
}

function OrgEvaluation({ org, days, teamLabels }: { org: string; days: number; teamLabels: ReadonlyMap<string, string> }) {
  const { t } = useI18n()
  const api = useApi()
  const evaluation = useQuery(evaluationQuery(api, org, days))
  const failure = evaluation.error instanceof ApiError ? evaluation.error : null
  if (evaluation.data === undefined) {
    if (evaluation.error === null) return <p className="muted">{t.evaluation.loading}</p>
    if (failure?.status === 404) return <p className="empty">{t.evaluation.unavailable}</p>
    return <p className="empty" role="alert">{failure?.status === 503 ? t.evaluation.couldNotAnswer(evaluation.error.message) : failureText(t, evaluation.error)}</p>
  }
  const report = evaluation.data
  return (
    <>
      {report.teams.length === 0 && (report.unavailable ?? [] /* stand-in-ok(optional): the node leaves the list out when every team answered, in an answer that has come */).length === 0 ? <p className="empty">{t.evaluation.noTeams}</p> : null}
      {report.teams.map((numbers) => (
        <TeamEvaluation key={numbers.team} label={teamLabels.get(numbers.team) ?? numbers.team} numbers={numbers} days={report.days} />
      ))}
      {(report.unavailable ?? [] /* stand-in-ok(optional): the node leaves the list out when every team answered, in an answer that has come */).map((one) => (
        <section key={one.team} className="ev-team">
          <h2 className="sub-head">{t.evaluation.team(teamLabels.get(one.team) ?? one.team)}</h2>
          <p className="empty">{t.evaluation.teamUnavailable(one.reason)}</p>
        </section>
      ))}
      <p className="muted ev-source">{t.evaluation.source(`/api/orgs/${encodeURIComponent(org)}/evaluation?days=${days}`)}</p>
    </>
  )
}
