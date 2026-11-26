import { Loading, Skeleton } from '../shared/Skeleton'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import type { DecisionEntry, DecisionState, Me } from '../api/clientApi'
import { useApi } from '../api/context'
import { decisionsQuery } from '../api/queries'
import { useI18n } from '../i18n'

/** How many decisions show before the rest fold away. */
const FOLD_AT = 6

const MINUTE = 60_000

type StateKind = DecisionState['kind']

function teamLabelOf(me: Me, entry: DecisionEntry): string {
  const org = me.orgs.find((one) => one.id === entry.org)
  const team = org?.teams.find((one) => one.id === entry.team)
  const teamLabel = team?.label ?? entry.team
  return me.orgs.length > 1 ? `${org?.label ?? entry.org} / ${teamLabel}` : teamLabel
}

/** What the node says about a decision that has not reached its team yet, or was refused. */
function StateDetail({ state }: { state: DecisionState }) {
  const { t, fmt } = useI18n()
  if (state.kind === 'refused') return <p className="decided-detail">{t.desk.recent.refusedDetail(state.message)}</p>
  if (state.kind !== 'queued') return null
  const untilRetry = Date.parse(state.nextAt) - Date.now()
  const retry = untilRetry <= 0 ? null : untilRetry < MINUTE ? t.desk.recent.underMinute : fmt.relative(state.nextAt, t.time.justNow)
  return (
    <p className="decided-detail">
      {t.desk.recent.queuedDetail(state.attempts, retry)}
      {state.lastError === undefined ? '' : ` ${t.desk.recent.lastError(state.lastError)}`}
    </p>
  )
}

function DecidedRow({ entry, teamLabel }: { entry: DecisionEntry; teamLabel: string }) {
  const { t, fmt } = useI18n()
  const kind: StateKind = entry.state.kind
  return (
    <li className={`decided state-${kind}`}>
      <div className="decided-top">
        <span className="decided-title">{entry.title ?? entry.target}</span>
        <span className={`decided-tag tag-${kind}`}>{t.desk.recent.states[kind]}</span>
      </div>
      <p className="decided-answer">
        {entry.choice === undefined ? null : <b>{t.desk.recent.chose(entry.choiceLabel ?? entry.choice)}</b>}
        {entry.choice !== undefined && entry.text !== '' ? ' ' : null}
        {entry.text}
      </p>
      <StateDetail state={entry.state} />
      <p className="decided-meta">
        <span>{teamLabel}</span>
        {entry.relayedFor === undefined ? null : <span>{t.desk.recent.answeredFor(entry.relayedFor)}</span>}
        <time dateTime={entry.decidedAt}>{fmt.relative(entry.decidedAt, t.time.justNow)}</time>
      </p>
    </li>
  )
}

/** The decisions made lately, with where each one stands: the answered requests have left the pile. */
export function RecentDecisions({ me }: { me: Me }) {
  const { t } = useI18n()
  const api = useApi()
  const log = useQuery(decisionsQuery(api))
  const [isExpanded, setIsExpanded] = useState(false)
  if (log.error !== null) return <p className="muted" role="alert">{t.desk.recent.unavailable(log.error.message)}</p>
  if (log.data === undefined) return <Loading quiet><Skeleton kind="box" /></Loading>
  const { entries } = log.data
  if (entries.length === 0) return null
  const shown = isExpanded ? entries : entries.slice(0, FOLD_AT)
  return (
    <section className="recent" aria-labelledby="recent-head">
      <h2 id="recent-head" className="pile-head">
        {t.desk.recent.head}
        <span className="count">{entries.length}</span>
      </h2>
      <ol className="decided-list">
        {shown.map((entry) => (
          <DecidedRow key={entry.id} entry={entry} teamLabel={teamLabelOf(me, entry)} />
        ))}
      </ol>
      {entries.length > shown.length ? (
        <button className="pile-more" type="button" onClick={() => setIsExpanded(true)}>
          {t.desk.pile.showMore(entries.length - shown.length)}
        </button>
      ) : null}
    </section>
  )
}
