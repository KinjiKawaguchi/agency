import { useI18n } from '../i18n'
import { printable } from '../shared/printable'
import type { OutboxEntry } from './entry'
import { useOutbox } from './OutboxProvider'
import './tray.css'

/** How many decisions are listed before the rest are counted. */
const LISTED = 4

const MINUTE = 60_000

function Entry({ entry }: { entry: OutboxEntry }) {
  const { t, fmt } = useI18n()
  const text = t.desk.tray
  const outbox = useOutbox()
  const untilRetry = entry.nextAt === undefined ? 0 : Date.parse(entry.nextAt) - Date.now()
  const retry = entry.nextAt === undefined || untilRetry <= 0 ? null : untilRetry < MINUTE ? t.desk.recent.underMinute : fmt.relative(entry.nextAt, t.time.justNow)
  const title = printable(entry.itemTitle)
  return (
    <li className={`tray-entry is-${entry.phase}`}>
      <span className="tray-title">{title}</span>
      {entry.chosenLabel === undefined ? null : <span className="tray-chose">{t.desk.recent.chose(entry.chosenLabel)}</span>}
      <span className="tray-state">{entry.phase === 'sending' ? text.sending : entry.phase === 'retrying' ? text.retrying(retry) : text.failed(entry.reason ?? '' /* stand-in-ok(optional): a failed send carries the reason the node or the browser gave; without one the line says only that it was not sent */)}</span>
      {entry.phase === 'retrying' && entry.reason !== undefined ? <span className="tray-reason muted">{entry.reason}</span> : null}
      {entry.phase === 'sending' ? null : (
        <span className="tray-actions">
          <button className="btn small" type="button" aria-label={text.sendAgainFor(title)} onClick={() => outbox.sendAgain(entry.id)}>
            {text.sendAgain}
          </button>
          {entry.phase === 'failed' ? (
            <button className="btn small" type="button" aria-label={text.discardFor(title)} onClick={() => outbox.discard(entry.id)}>
              {text.discard}
            </button>
          ) : null}
        </span>
      )}
    </li>
  )
}

/**
 * What is on its way to the node: a small list in a corner that never blocks the desk. Nothing is drawn while nothing is kept. A failure
 * stays with its reason until the owner sends it again (the same signed decision) or discards it; a decision leaves only when the node
 * has taken it.
 */
export function SendingTray() {
  const { t } = useI18n()
  const { entries } = useOutbox()
  if (entries.length === 0) return null
  const hasFailed = entries.some((entry) => entry.phase === 'failed')
  return (
    <section className="tray" role="region" aria-label={t.desk.tray.title} aria-live="polite">
      <h2 className="tray-head">
        {t.desk.tray.title} <span className="muted">({entries.length})</span>
      </h2>
      <ul>
        {entries.slice(0, LISTED).map((entry) => (
          <Entry key={entry.id} entry={entry} />
        ))}
      </ul>
      {entries.length > LISTED ? <p className="muted tray-more">{t.desk.tray.more(entries.length - LISTED)}</p> : null}
      {hasFailed ? <p className="muted tray-note">{t.desk.tray.discardNote}</p> : null}
    </section>
  )
}
