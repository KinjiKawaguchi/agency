import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useApi } from '../api/context'
import { failureText } from '../api/failure'
import type { KpiInfo } from '../api/outcomesApi'
import { numberHistoryQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { formatKpiValue } from './format'
import { replacedIn } from './numberInput'

/** Every number entered for a KPI, newest first, with where it came from; one that a later entry for its day replaced is marked, not hidden. */
export function NumberHistory({ org, kpi }: { org: string; kpi: KpiInfo }) {
  const { t, fmt, locale } = useI18n()
  const text = t.outcomes.history
  const api = useApi()
  const [isOpen, setIsOpen] = useState(false)
  const history = useQuery({ ...numberHistoryQuery(api, org, kpi.id), enabled: isOpen })
  const entries = history.data
  const replaced = entries === undefined ? [] : replacedIn(entries)
  return (
    <div className="oc-history">
      <button className="btn small" type="button" aria-expanded={isOpen} onClick={() => setIsOpen(!isOpen)}>
        {isOpen ? text.hide : text.show}
      </button>
      {!isOpen ? null : entries === undefined ? (
        history.error === null ? <Loading quiet><Skeleton kind="box" /></Loading> : <p className="empty" role="alert">{failureText(t, history.error)}</p>
      ) : entries.length === 0 ? (
        <p className="muted">{text.empty}</p>
      ) : (
        <ol className="oc-entries" aria-label={`${text.title}: ${kpi.label}`}>
          {entries.map((entry, index) => (
            <li key={`${entry.enteredAt}/${entry.date}/${index}`} className={replaced[index] === true ? 'is-replaced' : undefined}>
              <b>{fmt.shortDate(entry.date)}</b> {formatKpiValue(entry.value, kpi, locale, t.evaluation.units)}
              <span className="muted">
                {' · '}
                {entry.source.kind === 'manual' ? text.manual : text.connector(entry.source.name)}
                {' · '}
                {text.enteredOn(fmt.dateTime(entry.enteredAt))}
              </span>
              {entry.note === undefined ? null : <span className="oc-note"> {entry.note}</span>}
              {replaced[index] === true ? <i className="oc-replaced"> {text.replaced}</i> : null}
            </li>
          ))}
        </ol>
      )}
    </div>
  )
}
