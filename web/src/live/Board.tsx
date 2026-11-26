import type { Team } from '../api/teams'
import type { PersonStatus, WorkItem } from '../api/teamview'
import { useI18n } from '../i18n'
import { Face } from '../people/Face'
import { presenceOf } from '../people/state'

/** How many of the work items an agent holds are shown: those in progress (up to this many), or else the one next in line. */
const ITEMS_SHOWN = 2

const ORDER: readonly WorkItem['status'][] = ['inProgress', 'release', 'next', 'backlog', 'done']

interface Row {
  person: PersonStatus
  items: WorkItem[]
}

/** The agents of a team, those at work first, each with the work items they hold. */
function rowsOf(team: Team): Row[] {
  const { view } = team
  return view.people
    .filter((person) => person.kind === 'agent')
    .map((person) => ({ person, items: person.holding.flatMap((ref) => view.work.filter((item) => item.ref === ref)).toSorted((a, b) => ORDER.indexOf(a.status) - ORDER.indexOf(b.status)) }))
    .toSorted((a, b) => Number(b.person.state === 'working') - Number(a.person.state === 'working') || b.items.length - a.items.length)
}

/** What an agent is on now: the items in progress (a few), or else the one that is next. The rest are counted. */
const shown = (items: readonly WorkItem[]): WorkItem[] => {
  const going = items.filter((item) => item.status === 'inProgress').slice(0, ITEMS_SHOWN)
  return going.length > 0 ? going : items.slice(0, 1)
}

/** One agent: who, and what they are on (worked out once), with the epic of each item. */
function AgentRow({ team, person, items }: { team: Team; person: PersonStatus; items: WorkItem[] }) {
  const { t } = useI18n()
  const visible = shown(items)
  return (
    <li className="lv-agent">
      <Face label={person.label} kind="agent" state={presenceOf(person, team.waitingOnViewer)} size="sm" />
      <div className="lv-agent-body">
        <b>{person.label}</b>
        {visible.length === 0 ? (
          <span className="muted">{person.status === '' ? t.live.board.idle : person.status}</span>
        ) : (
          visible.map((item) => {
            const epic = team.view.epics.find((one) => one.ref === item.epic)
            return (
              <span key={item.ref} className="lv-item">
                <span className="lv-ref">{item.ref}</span> {item.title}
                {item.epic === undefined ? null : <span className="muted lv-epic">{t.live.board.epic(item.epic, epic?.title ?? item.epic)}</span>}
              </span>
            )
          })
        )}
        {items.length > visible.length && visible.length > 0 ? <span className="muted">{t.live.board.more(items.length - visible.length)}</span> : null}
      </div>
    </li>
  )
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
              <AgentRow key={person.id} team={team} person={person} items={items} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  )
}
