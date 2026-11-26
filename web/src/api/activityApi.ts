// The activity route of the Client API: what is happening in a team, for a screen left open all day.
// One feed for each team, read by the node from the team runtime (the agency-runtime/1 op `activity`) and served
// as `GET /api/orgs/<org>/teams/<team>/activity?since=<time>`. The types are generated from the Rust types in
// `bindings/` and are never edited by hand; only the client's own are defined here: each kind of event as a type.
import type { ActivityEvent } from '../../../bindings/ActivityEvent'

export type { Activity } from '../../../bindings/Activity'
export type { ActivityEvent } from '../../../bindings/ActivityEvent'

/** One kind of event, by its `kind`. */
export type EventOf<K extends ActivityEvent['kind']> = Extract<ActivityEvent, { kind: K }>

export type ItemAdded = EventOf<'itemAdded'>
export type StatusChanged = EventOf<'statusChanged'>
export type PrOpened = EventOf<'prOpened'>
export type PrMerged = EventOf<'prMerged'>
/** One agent wrote to another. Never the body: `summary` is the sender's own short label (at most 120 characters), and may be empty. */
export type Message = EventOf<'message'>
