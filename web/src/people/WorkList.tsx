import { useState } from 'react'
import type { TeamView, WorkItem } from '../api/teamview'
import { useI18n } from '../i18n'
import { Face } from './Face'
import { personOf } from './viewer'
import { blockedWork, inProgress } from './work'

function EtaText({ item }: { item: WorkItem }) {
  const { t, fmt } = useI18n()
  const { eta } = item
  if (eta === undefined) return <>{t.people.eta.none}</>
  if (eta.isOverBudget) return <>{t.people.eta.overBudget(eta.basis)}</>
  return (
    <>
      {t.people.eta.around(fmt.dateTime(eta.p50))}, {t.people.eta.latest(fmt.dateTime(eta.p80))}
    </>
  )
}

function WorkRow({ item, view }: { item: WorkItem; view: TeamView }) {
  const { t, fmt } = useI18n()
  const assignee = item.assignee === undefined ? undefined : personOf(view, item.assignee)
  const assigneeLabel = assignee?.label ?? item.assignee ?? t.people.unassigned
  return (
    <div className="work-row">
      <Face label={assigneeLabel} kind={assignee?.kind ?? 'agent'} size="sm" title={assigneeLabel} />
      <div className="work-body">
        <p className="work-title">{item.title}</p>
        <p className="work-meta">
          <span className="mono ref">{item.ref}</span>
          <span>{assigneeLabel}</span>
          {item.blocked === undefined ? null : (
            <span className="work-stop">
              {t.people.blockedReasons[item.blocked.reason]} {t.people.blockedSince(fmt.shortDate(item.blocked.since))}
            </span>
          )}
          {item.status === 'inProgress' ? <span title={item.eta?.basis ?? ''}><EtaText item={item} /></span> : null}
        </p>
      </div>
    </div>
  )
}

/** How many rows a list shows before the rest fold away. */
const ROWS_SHOWN = 8

function Rows({ items, view }: { items: readonly WorkItem[]; view: TeamView }) {
  const { t } = useI18n()
  const [isExpanded, setIsExpanded] = useState(false)
  const shown = isExpanded ? items : items.slice(0, ROWS_SHOWN)
  return (
    <div className="work">
      {shown.map((item) => (
        <WorkRow key={item.ref} item={item} view={view} />
      ))}
      {items.length > shown.length ? (
        <button className="work-more" type="button" onClick={() => setIsExpanded(true)}>
          {t.people.moreWork(items.length - shown.length)}
        </button>
      ) : null}
    </div>
  )
}

/** Work in progress, and below it the work that cannot move. */
export function WorkList({ view }: { view: TeamView }) {
  const { t } = useI18n()
  const doing = inProgress(view.work)
  const blocked = blockedWork(view.work)
  return (
    <>
      <h3 className="sub-head">{t.people.workTitle}</h3>
      {doing.length === 0 ? <p className="empty">{t.people.noWork}</p> : <Rows items={doing} view={view} />}
      {blocked.length === 0 ? null : (
        <>
          <h3 className="sub-head">{t.people.blockedTitle}</h3>
          <Rows items={blocked} view={view} />
        </>
      )}
    </>
  )
}
