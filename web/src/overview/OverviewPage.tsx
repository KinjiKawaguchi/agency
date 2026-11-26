import { useSuspenseTeamStates } from '../api/queries'
import { FundingCard } from '../costs/FundingCard'
import { useI18n } from '../i18n'
import { Region } from '../shared/Region'
import { Loading, Skeleton } from '../shared/Skeleton'
import { OverviewSkeleton } from './OverviewSkeleton'
import { orderTeams, orgRank, summarize } from './summary'
import { TeamCard } from './TeamCard'
import './overview.css'

/** Every org and team of the account on one screen, the ones that need the viewer first. */
export function OverviewPage() {
  const { t } = useI18n()
  return (
    <>
      <h1 className="page-title">{t.overview.title}</h1>
      <Region what={t.loading.what.overview} skeleton={<OverviewSkeleton />}>
        <Overview />
      </Region>
    </>
  )
}

/** The overview's content, which reads every team: it suspends until they have been read (a team that is not ready yet is retried), and shows a team that keeps failing in its place. */
function Overview() {
  const { t } = useI18n()
  const { me, orgs, loading } = useSuspenseTeamStates()
  const ready = orgs.flatMap(({ teams }) => teams.flatMap(({ state }) => (state.status === 'ready' ? [summarize(state.team)] : [])))
  const waiting = ready.reduce((sum, summary) => sum + summary.mine, 0)
  const waitingTeams = ready.filter((summary) => summary.mine > 0).length
  const ordered = orgs
    .map(({ org, teams }) => ({ org, teams: orderTeams(teams.map((entry) => ({ ...entry, label: entry.ref.label }))) }))
    .toSorted((a, b) => orgRank(a.teams) - orgRank(b.teams) || a.org.label.localeCompare(b.org.label))
  const now = Date.now()
  return (
    <>
      {loading.size > 0 ? <Loading quiet><p className="ov-lead"><Skeleton kind="line" /></p></Loading> : <p className="ov-lead">{t.overview.lead(waiting, waitingTeams)}</p>}
      {ordered.map(({ org, teams }) => (
        <section key={org.id} className="ov-org" aria-labelledby={`ov-org-${org.id}`}>
          <h2 className="sub-head" id={`ov-org-${org.id}`}>{org.label}</h2>
          {teams.length === 0 ? (
            <p className="empty">{t.overview.noTeams}</p>
          ) : (
            <div className="ov-teams">
              {teams.map(({ ref, state }) => (
                <TeamCard key={ref.id} orgId={org.id} teamRef={ref} state={state} />
              ))}
            </div>
          )}
        </section>
      ))}
      {me.funding.length === 0 ? null : (
        <section className="ov-org" aria-labelledby="ov-funding">
          <h2 className="sub-head" id="ov-funding">{t.overview.funding}</h2>
          <ul className="ov-funds">
            {me.funding.map((source) => (
              <li key={source.id}>
                <FundingCard source={source} now={now} />
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  )
}
