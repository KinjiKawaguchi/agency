import { useTeams } from '../api/queries'
import { teamKey, type Team } from '../api/teams'
import { useI18n } from '../i18n'
import { EpicList } from './EpicList'
import { PersonCard } from './PersonCard'
import { presenceOf } from './state'
import { ReplicaNotice } from '../shared/ReplicaNotice'
import { WorkList } from './WorkList'
import './people.css'

function TeamSection({ team }: { team: Team }) {
  const { t } = useI18n()
  const { view, viewer } = team
  const others = view.people.filter((person) => person.id !== viewer)
  const working = others.filter((person) => person.state === 'working').length
  const waiting = others.filter((person) => presenceOf(person, viewer) === 'needs-you').length
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
  const { teams, isLoading, error } = useTeams()
  if (error !== null && teams.length === 0) return <p className="empty" role="alert">{t.app.unreachable(error.message)}</p>
  if (isLoading && teams.length === 0) return <p className="muted">{t.app.loading}</p>
  return (
    <>
      <h1 className="page-title">{t.people.title}</h1>
      {teams.length === 0 ? <p className="empty">{t.people.noTeams}</p> : teams.map((team) => <TeamSection key={teamKey(team)} team={team} />)}
    </>
  )
}
