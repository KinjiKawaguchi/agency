// The activity answer comes from the node, so it is read once here before the screen sees it. A feed is read
// tolerantly: an event of a kind this client does not know, or one that is malformed, is left out and does not
// spoil the rest, since a feed that is left open all day should keep running when the node learns a new kind of event.
import { MalformedAnswer } from './evaluationAnswers'
import type { Activity, ActivityEvent } from './activityApi'
import type { WorkStatus } from './teamview'

type Raw = Record<string, unknown>

const asObject = (value: unknown): Raw => (typeof value === 'object' && value !== null && !Array.isArray(value) ? (value as Raw) : {})
const text = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)
const STATUSES: readonly string[] = ['backlog', 'next', 'inProgress', 'release', 'done']
const status = (value: unknown): WorkStatus | undefined => (typeof value === 'string' && STATUSES.includes(value) ? (value as WorkStatus) : undefined)
/** An optional string field as a property that is there only when there is something in it. */
const maybe = <K extends string>(key: K, value: unknown): { [P in K]?: string } => (text(value) === undefined ? {} : ({ [key]: text(value) } as { [P in K]?: string }))

function eventOf(raw: unknown): ActivityEvent | undefined {
  const given = asObject(raw)
  const id = text(given.id)
  const at = text(given.at)
  if (id === undefined || at === undefined || Number.isNaN(Date.parse(at))) return undefined
  const base = { id, at }
  switch (given.kind) {
    case 'itemAdded': {
      const ref = text(given.ref)
      const title = text(given.title)
      return ref === undefined || title === undefined ? undefined : { ...base, kind: 'itemAdded', ref, title, ...maybe('assignee', given.assignee), ...maybe('epic', given.epic) }
    }
    case 'statusChanged': {
      const [ref, title, from, to] = [text(given.ref), text(given.title), status(given.from), status(given.to)]
      return ref === undefined || title === undefined || from === undefined || to === undefined ? undefined : { ...base, kind: 'statusChanged', ref, title, from, to, ...maybe('epic', given.epic) }
    }
    case 'prOpened': {
      const ref = text(given.ref)
      const title = text(given.title)
      return ref === undefined || title === undefined ? undefined : { ...base, kind: 'prOpened', ref, title, ...maybe('by', given.by), ...maybe('issue', given.issue), ...maybe('epic', given.epic) }
    }
    case 'prMerged': {
      const ref = text(given.ref)
      const title = text(given.title)
      if (ref === undefined || title === undefined) return undefined
      const lead = typeof given.leadSeconds === 'number' && Number.isFinite(given.leadSeconds) && given.leadSeconds >= 0 ? { leadSeconds: given.leadSeconds } : {}
      return { ...base, kind: 'prMerged', ref, title, ...maybe('by', given.by), ...maybe('issue', given.issue), ...maybe('epic', given.epic), ...lead }
    }
    case 'message': {
      const [from, to] = [text(given.from), text(given.to)]
      return from === undefined || to === undefined ? undefined : { ...base, kind: 'message', from, to, summary: typeof given.summary === 'string' ? given.summary.slice(0, 120) : '' }
    }
    default:
      return undefined
  }
}

/** The answer as the screen uses it: the events it knows, oldest first. An answer with no org, team, or time is refused. */
export function readActivity(raw: unknown): Activity {
  const given = asObject(raw)
  const [org, team, generatedAt] = [text(given.org), text(given.team), text(given.generatedAt)]
  if (org === undefined || team === undefined || generatedAt === undefined) throw new MalformedAnswer('activity header')
  const events = (Array.isArray(given.events) ? given.events : []).flatMap((one) => {
    const event = eventOf(one)
    return event === undefined ? [] : [event]
  })
  return { org, team, generatedAt, events: events.toSorted((a, b) => Date.parse(a.at) - Date.parse(b.at)), truncated: given.truncated === true }
}
