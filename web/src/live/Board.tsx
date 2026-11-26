import type { Team } from '../api/teams'
import type { PersonStatus, WorkItem } from '../api/teamview'
import { useI18n } from '../i18n'
import { Face } from '../people/Face'
import { presenceOf } from '../people/state'

const ITEMS_SHOWN = 2

interface Row {
  person: PersonStatus
  items: WorkItem[]
}

/** The agents of a team, those at work first, each with the work items they hold. */
function rowsOf(team: Team): Row[] {
  const { view } = team
  return view.people
    .filter((person) => person.kind === 'agent')
    .map((person) => ({ person, items: person.holding.flatMap((ref) => view.work.filter((item) => item.ref === ref)) }))
    .toSorted((a, b) => Number(b.person.state === 'working') - Number(a.person.state === 'working') || b.items.length - a.items.length)
}

/** What each agent works on now, and the epic that work belongs to. */
export function Board({ teams }: { teams: readonly Team[] }) {
  const { t } = useI18n()
  return (
    <div className="lv-board">
      {teams.map((team) => (
        <section key={`${team.view.org}/${team.view.team}`} aria-label={t.people.team(team.view.label)}>
          <h3 className="lv-board-team">{team.view.label}</h3>
          <ul>
            {rowsOf(team).map(({ person, items }) => (
              <li key={person.id} className="lv-agent">
                <Face label={person.label} kind="agent" state={presenceOf(person, team.waitingOnViewer)} size="sm" />
                <div className="lv-agent-body">
                  <b>{person.label}</b>
                  {items.length === 0 ? (
                    <span className="muted">{person.status === '' ? t.live.board.idle : person.status}</span>
                  ) : (
                    items.slice(0, ITEMS_SHOWN).map((item) => {
                      const epic = team.view.epics.find((one) => one.ref === item.epic)
                      return (
                        <span key={item.ref} className="lv-item">
                          <span className="lv-ref">{item.ref}</span> {item.title}
                          {item.epic === undefined ? null : <span className="muted lv-epic">{t.live.board.epic(item.epic, epic?.title ?? item.epic)}</span>}
                        </span>
                      )
                    })
                  )}
                  {items.length > ITEMS_SHOWN ? <span className="muted">{t.live.board.more(items.length - ITEMS_SHOWN)}</span> : null}
                </div>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  )
}
