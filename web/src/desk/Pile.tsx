import { Loading, Skeleton } from '../shared/Skeleton'
import { useState } from 'react'
import { Face } from '../people/Face'
import { useI18n } from '../i18n'
import { GROUP_ORDER, blockedPeople, type DeskItem, type GroupId } from './items'

const FOLD_AT = 5

interface PileProps {
  items: readonly DeskItem[]
  selected: string | undefined
  /** Shows each item's organization, for people who belong to more than one. */
  orgLabelOf: ((org: string) => string) | null
  onSelect: (key: string) => void
}

function PileRow({ item, isSelected, orgLabel, onSelect }: { item: DeskItem; isSelected: boolean; orgLabel: string | null; onSelect: () => void }) {
  const { t, fmt } = useI18n()
  const drafterLabel = item.drafter?.label ?? item.request.from
  return (
    <button className="pile-item" type="button" role="option" aria-selected={isSelected} onClick={onSelect}>
      <Face label={drafterLabel} kind={item.drafter?.kind ?? 'agent'} size="sm" />
      <span className="pile-body">
        <span className={`pile-kind kind-${item.request.kind}`}>{t.desk.pile.kinds[item.request.kind]}</span>
        <span className="pile-title">{item.request.title}</span>
        <span className="pile-meta">
          <span>{drafterLabel}</span>
          {orgLabel === null ? null : <span>{orgLabel}</span>}
          <span>{fmt.relative(item.request.createdAt, t.time.justNow)}</span>
          {blockedPeople(item).length > 0 ? <span className="pile-blocks">{t.desk.pile.blocking(blockedPeople(item).length)}</span> : null}
        </span>
      </span>
    </button>
  )
}

function PileGroup({ id, items, selected, orgLabelOf, onSelect }: PileProps & { id: GroupId }) {
  const { t } = useI18n()
  const [isExpanded, setIsExpanded] = useState(false)
  const selectedIsHidden = items.slice(FOLD_AT).some((item) => item.key === selected)
  const isFolded = items.length > FOLD_AT + 1 && !isExpanded && !selectedIsHidden
  const shown = isFolded ? items.slice(0, FOLD_AT) : items
  return (
    <div className="pile-group">
      <h2 className="pile-head">
        {t.desk.pile.groups[id]}
        <span className="count">{items.length}</span>
      </h2>
      {shown.map((item) => (
        <PileRow key={item.key} item={item} isSelected={item.key === selected} orgLabel={orgLabelOf?.(item.view.org) ?? null} onSelect={() => onSelect(item.key)} />
      ))}
      {isFolded ? (
        <button className="pile-more" type="button" onClick={() => setIsExpanded(true)}>
          {t.desk.pile.showMore(items.length - FOLD_AT)}
        </button>
      ) : null}
    </div>
  )
}

/** What stands in the pile while the teams are read: some rows, or one under the rows of the teams that have answered. */
export function PileSkeleton({ rows = 3 }: { rows?: number }) {
  return (
    <Loading quiet={rows === 1}>
      <div className="pile-skeleton">
        {Array.from({ length: rows }, (_, index) => (
          <Skeleton key={index} kind="box" />
        ))}
      </div>
    </Loading>
  )
}

export function Pile(props: PileProps) {
  const { t } = useI18n()
  if (props.items.length === 0) return <p className="empty">{t.desk.pile.empty}</p>
  return (
    <div className="pile" role="listbox" aria-label={t.desk.pile.label}>
      {GROUP_ORDER.map((id) => {
        const items = props.items.filter((item) => item.group === id)
        return items.length === 0 ? null : <PileGroup key={id} {...props} id={id} items={items} />
      })}
    </div>
  )
}
