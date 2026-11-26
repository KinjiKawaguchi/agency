import { useSuspenseTeamStates } from '../api/queries'
import { teamKey, type Team } from '../api/teams'
import { useI18n } from '../i18n'
import { EpicList } from './EpicList'
import { PersonCard } from './PersonCard'
import { presenceOf } from './state'
import { MateSkeletons, PeopleSkeleton } from './PeopleSkeleton'
import { Region } from '../shared/Region'
import { ReplicaNotice } from '../shared/ReplicaNotice'
import { TeamFailed } from '../shared/TeamFailed'
import { WorkList } from './WorkList'
import './people.css'

function TeamSection({ team }: { team: Team }) {
  const { t } = useI18n()
  const { view, viewer, waitingOnViewer } = team
  const others = view.people.filter((person) => person.id !== viewer)
  const working = others.filter((person) => person.state === 'working').length
  const waiting = others.filter((person) => presenceOf(person, waitingOnViewer) === 'needs-you').length
  return (
    <section className="crew">
      <header className="crew-head">
        <h2>{t.people.team(view.label)}</h2>
        <p>
          {t.people.summary(working, others.length)} {waiting > 0 ? t.people.waitingOnYou(waiting) : ''}
        </p>
      </header>
      <ReplicaNotice source={team.source} />
      <div className="people">
        {others.map((person) => (
          <PersonCard key={person.id} person={person} team={team} />
        ))}
      </div>
      <WorkList view={view} />
      <EpicList epics={view.epics} />
    </section>
  )
}

export function PeoplePage() {
  const { t } = useI18n()
  return (
    <>
      <h1 className="page-title">{t.people.title}</h1>
      <Region what={t.loading.what.people} skeleton={<PeopleSkeleton />}>
        <People />
      </Region>
    </>
  )
}

/** Every team's participants and work. It suspends until the teams have been read; a team that keeps failing is a section that says so. */
function People() {
  const { t } = useI18n()
  const { orgs } = useSuspenseTeamStates()
  const entries = orgs.flatMap(({ org, teams }) => teams.map((entry) => ({ org: org.id, ...entry })))
  if (entries.length === 0) return <p className="empty">{t.people.noTeams}</p>
  return (
    <>
      {entries.map(({ org, ref, state }) =>
        state.status === 'ready' ? (
          <TeamSection key={teamKey(state.team)} team={state.team} />
        ) : state.status === 'failed' ? (
          <section key={`${org}/${ref.id}`} className="crew">
            <header className="crew-head">
              <h2>{t.people.team(ref.label)}</h2>
            </header>
            <TeamFailed org={org} team={ref.id} failure={state} />
          </section>
        ) : (
          <section key={`${org}/${ref.id}`} className="crew">
            <header className="crew-head">
              <h2>{t.people.team(ref.label)}</h2>
            </header>
            <MateSkeletons />
          </section>
        ),
      )}
    </>
  )
}
