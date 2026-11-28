import { useQuery } from '@tanstack/react-query'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import { aboutQuery, effortQuery, orgDefinitionQuery } from '../api/queries'
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
  // The node says whether it serves this: a node that does not list the capability is not asked.
  const about = useQuery(aboutQuery(api))
  const hasEffort = about.data?.capabilities.includes('effort') === true
  const effort = useQuery({ ...effortQuery(api, org, days), enabled: hasEffort })
  const definition = useQuery(orgDefinitionQuery(api, org))
  const failure = effort.error instanceof ApiError ? effort.error : null
  const planFor = (team: string): PlanState => {
    if (definition.data === undefined) return definition.isPending ? { kind: 'loading' } : { kind: 'unreadable' }
    const plan = planOf(definition.data, team)
    return plan === undefined ? { kind: 'none' } : { kind: 'stated', plan }
  }
  return (
    <section className="ev-effort">
      <h2 className="sub-head">{t.evaluation.effort.title}</h2>
      <p className="muted ev-lead">{t.evaluation.effort.lead}</p>
      {about.data === undefined ? (
        about.error === null ? <p className="muted">{t.evaluation.loading}</p> : <p className="empty" role="alert">{failureText(t, about.error)}</p>
      ) : !hasEffort ? (
        <p className="empty">{t.evaluation.effort.unavailable}</p>
      ) : effort.data === undefined ? (
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
            <TeamEffortView key={team.team} org={org} label={teamLabels.get(team.team) ?? team.team} effort={team} plan={planFor(team.team)} days={effort.data.days} />
          ))}
          {(effort.data.unavailable ?? [] /* stand-in-ok(optional): the node leaves the list out when every team answered, in an answer that has come */).map((one) => (
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
