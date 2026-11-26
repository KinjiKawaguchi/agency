// The activity route of the Client API: what is happening in a team, for a screen left open all day.
// One feed for each team, read by the node from the team runtime (the agency-runtime/1 op `activity`) and served
// as `GET /api/orgs/<org>/teams/<team>/activity?since=<time>`. AgencyHarness has agreed this shape; AgencyCore2
// owns the Rust type, and until it is generated these types stand in for it (the same temporary exception as
// the evaluation types had before #150).
import type { WorkStatus } from './teamview'

interface Base {
  /** Short, stable, and unique in the team (`m-` for a message, `b-` for a board event), so an overlap is not shown twice. */
  id: string
  /** RFC 3339. The time of the thing: exact for an item added and a pull request opened or merged; for a status change, when the harness first saw it. */
  at: string
}

/** An issue or a task appeared on the board. */
export interface ItemAdded extends Base {
  kind: 'itemAdded'
  /** `repo#N` */
  ref: string
  title: string
  /** The parent epic's ref, when the harness could read it. */
  epic?: string
}

export interface StatusChanged extends Base {
  kind: 'statusChanged'
  ref: string
  title: string
  from: WorkStatus
  to: WorkStatus
  epic?: string
}

export interface PrOpened extends Base {
  kind: 'prOpened'
  ref: string
  title: string
  /** A participant's name, only when the pull request's author is one: usually absent. */
  by?: string
  /** The issue the pull request names, filled for recent ones. */
  issue?: string
  epic?: string
}

export interface PrMerged extends Base {
  kind: 'prMerged'
  ref: string
  title: string
  by?: string
  issue?: string
  epic?: string
  /** Opened to merged, in seconds. */
  leadSeconds?: number
}

/** One agent wrote to another. Never the body: `summary` is the sender's own short label (at most 120 characters), and may be empty. */
export interface Message extends Base {
  kind: 'message'
  /** Participant names, as in a team view's `people[].id`. */
  from: string
  to: string
  summary: string
}

export type ActivityEvent = ItemAdded | StatusChanged | PrOpened | PrMerged | Message

export interface Activity {
  org: string
  team: string
  generatedAt: string
  /** Oldest first, each strictly after the `since` asked for. */
  events: ActivityEvent[]
  /** The list was cut at the limit: ask again from the last event's time. */
  truncated: boolean
}
