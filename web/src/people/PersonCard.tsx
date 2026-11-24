import { Link } from '@tanstack/react-router'
import type { Team } from '../api/teams'
import type { PersonStatus } from '../api/teamview'
import { useI18n } from '../i18n'
import { Face } from './Face'
import { presenceOf } from './state'

/** One person on a team: their face, what they are doing, and what they hold. */
export function PersonCard({ person, team }: { person: PersonStatus; team: Team }) {
  const { t, fmt } = useI18n()
  const presence = presenceOf(person, team.viewer)
  const waitingFor = presence === 'needs-you' ? team.view.requests.filter((request) => request.blocks.includes(person.id)) : []
  return (
    <article className={`mate presence-${presence}`}>
      <Face label={person.label} kind={person.kind} state={presence} size="lg" />
      <div className="mate-body">
        <div className="mate-top">
          <b className="mate-name">{person.label}</b>
          <span className="mate-role">{t.people.roles[person.role]}</span>
          {presence === 'idle' ? null : <span className={`mate-tag tag-${presence}`}>{t.people.stateTags[presence]}</span>}
        </div>
        <p className="mate-now">{person.status}</p>
        {presence === 'needs-you' ? (
          <p className="mate-sub">
            {t.people.waitingForAnswer}
            {waitingFor[0] === undefined ? '' : `: “${waitingFor[0].title}”`}
          </p>
        ) : null}
        {person.since === undefined ? null : <p className="mate-since">{t.people.since(fmt.dateTime(person.since))}</p>}
        {person.holding.length === 0 ? null : (
          <p className="mate-holding">
            {t.people.holding}: {person.holding.map((ref) => <span key={ref} className="mono ref">{ref}</span>)}
          </p>
        )}
        {presence === 'needs-you' ? (
          <Link className="mate-go" to="/desk">
            {t.people.openDecisions}
          </Link>
        ) : null}
      </div>
    </article>
  )
}
