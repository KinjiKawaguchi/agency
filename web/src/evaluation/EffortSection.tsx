import { useQuery } from '@tanstack/react-query'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { effortQuery, orgDefinitionQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { planOf } from './effort'
import { TeamEffortView, type PlanState } from './TeamEffortView'

interface Props {
  org: string
  days: number
  teamLabels: ReadonlyMap<string, string>
}

/** Where each team's effort goes, by work type, against the plan in the org definition. */
export function EffortSection({ org, days, teamLabels }: Props) {
  const { t } = useI18n()
  const api = useApi()
  const effort = useQuery(effortQuery(api, org, days))
  const definition = useQuery(orgDefinitionQuery(api, org))
  const failure = effort.error instanceof ApiError ? effort.error : null
  const planFor = (team: string): PlanState => {
    if (definition.data === undefined) return definition.isPending ? { kind: 'none' } : { kind: 'unreadable' }
    const plan = planOf(definition.data, team)
    return plan === undefined ? { kind: 'none' } : { kind: 'stated', plan }
  }
  return (
    <section className="ev-effort">
      <h2 className="sub-head">{t.evaluation.effort.title}</h2>
      <p className="muted ev-lead">{t.evaluation.effort.lead}</p>
      {effort.data === undefined ? (
        effort.error === null ? (
          <p className="muted">{t.evaluation.loading}</p>
        ) : failure?.status === 404 ? (
          <p className="empty">{t.evaluation.effort.unavailable}</p>
        ) : (
          <p className="empty" role="alert">{failure?.status === 503 ? t.evaluation.effort.couldNotAnswer(effort.error.message) : failureText(t, effort.error)}</p>
        )
      ) : (
        <>
          {effort.data.teams.map((team) => (
            <TeamEffortView key={team.team} label={teamLabels.get(team.team) ?? team.team} effort={team} plan={planFor(team.team)} days={effort.data.days} generatedAt={effort.data.generatedAt} until={effort.data.until} />
          ))}
          {(effort.data.unavailable ?? []).map((one) => (
            <section key={one.team} className="ev-team">
              <h3 className="sub-head">{t.evaluation.team(teamLabels.get(one.team) ?? one.team)}</h3>
              <p className="empty">{t.evaluation.teamUnavailable(one.reason)}</p>
            </section>
          ))}
          <p className="muted ev-source">{t.evaluation.effort.source(`/api/orgs/${encodeURIComponent(org)}/effort?days=${days}`)}</p>
        </>
      )}
    </section>
  )
}
