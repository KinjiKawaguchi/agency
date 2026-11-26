import { useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { ApiError } from '../api/client'
import { useApi } from '../api/context'
import { usageQuery, type TeamState } from '../api/queries'
import type { TeamRef } from '../api/clientApi'
import type { Team } from '../api/teams'
import { teamKey } from '../api/teams'
import { totalsOf } from '../costs/usage'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { TeamFailed } from '../shared/TeamFailed'
import { summarize } from './summary'

const USAGE_DAYS = 7

/** The week's API-priced cost, from the team's runtime. Left out for a team whose node is known not to answer. */
function Usage({ team }: { team: Team }) {
  const { t, fmt } = useI18n()
  const api = useApi()
  const { view } = team
  const usage = useQuery(usageQuery(api, view.org, view.team, USAGE_DAYS))
  if (usage.data !== undefined) return <p className="ov-line">{t.overview.usage(fmt.money(totalsOf(usage.data).costUsd))}</p>
  if (usage.error === null) return null
  return <p className="ov-line ov-quiet">{usage.error instanceof ApiError && usage.error.status === 404 ? t.overview.usageNone : t.overview.usageFailed}</p>
}

/** When the data is a copy, how old it is and why it could not be renewed. */
function DataAge({ team }: { team: Team }) {
  const { t, fmt } = useI18n()
  const { source } = team
  if (source.kind !== 'replica') return null
  const isStale = source.error !== undefined
  if (source.fetchedAt === undefined) return <p className="ov-age is-stale">{t.overview.neverRead(source.error ?? '')}</p>
  const relative = fmt.relative(source.fetchedAt, t.time.justNow)
  return <p className={`ov-age${isStale ? ' is-stale' : ''}`} title={fmt.dateTime(source.fetchedAt)}>{isStale ? t.overview.staleFrom(relative, source.error ?? '') : t.overview.dataFrom(relative)}</p>
}

function Body({ team }: { team: Team }) {
  const { t } = useI18n()
  const summary = summarize(team)
  const { view } = team
  return (
    <>
      <DataAge team={team} />
      {summary.mine > 0 ? (
        <Link className="ov-mine" to="/desk" search={{ team: teamKey(team) }}>
          {t.overview.waitingForYou(summary.mine)}
        </Link>
      ) : (
        <p className="ov-line ov-quiet">{t.overview.nothingWaits}</p>
      )}
      <p className="ov-line">{summary.participants === 0 ? t.overview.noParticipants : t.overview.working(summary.working, summary.participants)}</p>
      {summary.blockedCount > 0 ? (
        <div className="ov-why">
          <p className="ov-line"><b>{t.overview.blocked(summary.blockedCount)}</b></p>
          <ul>
            {summary.blocked.map((entry) => (
              <li key={entry.reason}>{t.people.blockedReasons[entry.reason]} ({entry.count})</li>
            ))}
          </ul>
        </div>
      ) : null}
      {summary.attention === 'idle' ? (
        <div className="ov-why">
          <p className="ov-line"><b>{t.overview.nobodyWorking}</b></p>
          <ul>
            {summary.quiet.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      ) : null}
      {team.source.kind === 'replica' && team.source.error !== undefined ? null : <Usage key={view.team} team={team} />}
    </>
  )
}

export function TeamCard({ orgId, teamRef, state }: { orgId: string; teamRef: TeamRef; state: TeamState }) {
  const { t } = useI18n()
  const summary = state.status === 'ready' ? summarize(state.team) : undefined
  const tag = state.status === 'failed' ? 'failed' : summary?.attention === 'active' ? undefined : summary?.attention
  return (
    <article className={`ov-team attention-${state.status === 'failed' ? 'failed' : (summary?.attention ?? 'loading')}`} aria-label={`${orgId}/${teamRef.id}`}>
      <header className="ov-team-head">
        <h3>{teamRef.label}</h3>
        {tag === undefined ? null : <span className={`ov-tag tag-${tag}`}>{t.overview.tags[tag]}</span>}
      </header>
      {state.status === 'ready' ? <Body team={state.team} /> : null}
      {state.status === 'loading' ? (
        <Loading>
          <Skeleton />
          <Skeleton kind="short" />
        </Loading>
      ) : null}
      {state.status === 'failed' ? <TeamFailed org={orgId} team={teamRef.id} failure={state} /> : null}
    </article>
  )
}
