import { Link } from '@tanstack/react-router'
import type { Me } from '../api/clientApi'
import type { TeamView } from '../api/teamview'
import type { Around } from '../i18n/en'
import { useI18n } from '../i18n'
import { Face } from '../people/Face'
import { OWNER_NAME, presenceOf, type Presence } from '../people/state'
import type { DeskItem, Situation } from './items'

interface BriefingProps {
  me: Me
  views: readonly TeamView[]
  items: readonly DeskItem[]
  situation: Situation
  now: Date
}

const RING_ORDER: readonly Presence[] = ['needs-you', 'working', 'waiting', 'idle', 'away']

function FaceRow({ views }: { views: readonly TeamView[] }) {
  const { t } = useI18n()
  const people = views
    .flatMap((view) => view.people)
    .filter((person) => person.id !== OWNER_NAME)
    .toSorted((a, b) => RING_ORDER.indexOf(presenceOf(a)) - RING_ORDER.indexOf(presenceOf(b)))
  return (
    <Link className="face-row" to="/people" aria-label={t.desk.briefing.seePeople}>
      {people.map((person) => (
        <Face key={person.id} label={person.label} kind={person.kind} state={presenceOf(person)} title={`${person.label}: ${t.person.states[presenceOf(person)]}`} />
      ))}
    </Link>
  )
}

function Next({ top }: { top: DeskItem }) {
  const { t } = useI18n()
  const link = (
    <Link className="inline-link" to="/desk" search={{ item: top.key }} replace>
      “{top.request.title}”
    </Link>
  )
  const [before, after]: Around =
    top.request.kind === 'proposal'
      ? t.desk.briefing.nextProposal
      : top.request.kind === 'question'
        ? t.desk.briefing.nextQuestion(top.drafter?.label ?? top.request.from)
        : t.desk.briefing.nextPending
  return (
    <>
      <br />
      {before}
      {link}
      {after}
    </>
  )
}

/** The one-line summary at the top: who is waiting for the viewer, and what to decide first. */
export function Briefing({ me, views, items, situation, now }: BriefingProps) {
  const { t, fmt } = useI18n()
  const { desk } = t
  const top = items.find((item) => item.isMine)
  const first =
    situation.waitingOnYou > 0
      ? desk.briefing.someoneWaiting(situation.waitingOnYou)
      : situation.mine > 0
        ? desk.briefing.itemsOnDesk(situation.mine)
        : desk.briefing.nothingToDecide
  const when = `${fmt.day(now.toISOString())} ${fmt.clock(now.toISOString())}`
  return (
    <section className="briefing">
      <p className="briefing-date">{desk.briefing.greeting(me.account.label, when)}</p>
      <h1 className="briefing-line">
        {first}
        {top === undefined ? null : <Next top={top} />}
      </h1>
      <div className="briefing-foot">
        <FaceRow views={views} />
        <span className="briefing-sub">
          {desk.briefing.peopleWorking(situation.working, situation.total)}
          {situation.mine > 1 ? ` ${desk.briefing.allOnDesk(situation.mine)}` : ''}
        </span>
      </div>
    </section>
  )
}
