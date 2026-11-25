import { Link } from '@tanstack/react-router'
import { useState } from 'react'
import type { Team } from '../api/teams'
import type { PersonStatus } from '../api/teamview'
import { useI18n } from '../i18n'
import { Face } from './Face'
import { presenceOf } from './state'

/** How many refs a person's card shows before the rest fold away. */
const HELD_SHOWN = 6

function Holding({ refs }: { refs: readonly string[] }) {
  const { t } = useI18n()
  const [isExpanded, setIsExpanded] = useState(false)
  const shown = isExpanded ? refs : refs.slice(0, HELD_SHOWN)
  return (
    <p className="mate-holding">
      <span>{t.people.holding}:</span>
      {shown.map((ref) => (
        <span key={ref} className="mono ref">{ref}</span>
      ))}
      {refs.length > shown.length ? (
        <button className="fold-more" type="button" onClick={() => setIsExpanded(true)}>
          {t.people.moreHeld(refs.length - shown.length)}
        </button>
      ) : null}
    </p>
  )
}

/** One person on a team: their face, what they are doing, and what they hold. */
export function PersonCard({ person, team }: { person: PersonStatus; team: Team }) {
  const { t, fmt } = useI18n()
  const presence = presenceOf(person, team.waitingOnViewer)
  const waitingFor = presence === 'needs-you' ? team.view.requests.filter((request) => request.blocks.includes(person.id)) : []
  return (
    <article className={`mate presence-${presence}`}>
      <Face label={person.label} kind={person.kind} state={presence} size="lg" />
      <div className="mate-body">
        <div className="mate-top">
          <b className="mate-name">{person.label}</b>
          {person.role === 'lead' ? <span className="mate-role">{t.people.roles.lead}</span> : null}
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
          <Holding refs={person.holding} />
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
