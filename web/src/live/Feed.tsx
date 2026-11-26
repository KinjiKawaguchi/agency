import type { ActivityEvent } from '../api/activityApi'
import { useI18n } from '../i18n'
import { formatDuration } from '../evaluation/duration'
import type { TaggedEvent } from './stream'

const SHOWN = 60

interface Props {
  events: readonly TaggedEvent[]
  /** The events that have just arrived: they come in with a stir (none where motion is not wanted). */
  fresh: readonly TaggedEvent[]
  teamLabels: ReadonlyMap<string, string>
}

function useDescribe(): (event: ActivityEvent) => { text: string; note?: string } {
  const { t } = useI18n()
  const text = t.live.feed
  return (event) => {
    switch (event.kind) {
      case 'itemAdded':
        return { text: text.added(event.ref, event.title) }
      case 'statusChanged':
        return { text: text.changed(event.ref, event.title, t.live.status[event.from], t.live.status[event.to]) }
      case 'prOpened':
        return { text: text.opened(event.ref, event.title) }
      case 'prMerged':
        return { text: text.merged(event.ref, event.title), ...(event.leadSeconds === undefined ? {} : { note: text.leadTime(formatDuration(event.leadSeconds, t.evaluation.units)) }) }
      case 'message':
        return { text: text.message(event.from, event.to), ...(event.summary === '' ? {} : { note: event.summary }) }
    }
  }
}

/** What is happening, newest first: items added, status changes, pull requests opened and merged, and messages between agents. */
export function Feed({ events, fresh, teamLabels }: Props) {
  const { t, fmt } = useI18n()
  const describe = useDescribe()
  const isFresh = new Set(fresh.map((one) => one.event.id))
  const shown = events.slice(-SHOWN).toReversed()
  if (shown.length === 0) return <p className="muted lv-empty">{t.live.feed.empty}</p>
  return (
    <ol className="lv-feed" aria-label={t.live.feed.title}>
      {shown.map(({ org, team, event }) => {
        const line = describe(event)
        return (
          <li key={`${org}/${team}/${event.id}`} className={`lv-row lv-kind-${event.kind}${isFresh.has(event.id) ? ' lv-new' : ''}`}>
            <span className="lv-kind" aria-hidden="true" />
            <span className="lv-time">{fmt.clock(event.at)}</span>
            <span className="lv-what">
              {line.text}
              {line.note === undefined ? null : <span className="muted"> — {line.note}</span>}
            </span>
            <span className="lv-team">{teamLabels.get(`${org}/${team}`) ?? team}</span>
          </li>
        )
      })}
    </ol>
  )
}
