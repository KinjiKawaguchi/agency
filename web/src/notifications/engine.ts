// What is new, as the web client and the iPhone app both say it (agency#167, `testdata/notifications.json`).
// A client keeps the last answer it read of each source. An event is the difference between that answer and the
// next one. The first good read of a source is its baseline and makes no event; a read that failed makes none and
// leaves the last answer where it was, so the next good read is not all new.

export type EventKind = 'request' | 'participant' | 'blocked' | 'device' | 'decision'

export interface NoticeEvent {
  kind: EventKind
  /** Stable, so that the same event is never shown twice: `request:org/team/id`, `participant:org/team/id`, `blocked:org/team/ref`, `device:public key`, `decision:id:state`. */
  key: string
}

/** What this client is: its own device key, whether it can approve other devices, and the decisions it sent in this session. */
export interface Own {
  deviceKey: string
  canApprove: boolean
  sentDecisionIds: readonly string[]
}

export interface TeamReading {
  org: string
  team: string
  /** False when the team could not be read: the rest is ignored. */
  ok: boolean
  /** The ids of the requests that are the viewer's (the node's `mine`). */
  mine: readonly string[]
  /** The ids of the participants who wait on the viewer (the node's `waitingOnViewer`). */
  waitingOnViewer: readonly string[]
  blocked: ReadonlyArray<{ ref: string; reason: string }>
}

export interface Reading {
  teams: readonly TeamReading[]
  /** The public keys of the devices that wait to be added; `null` when the list could not be read. */
  devices: readonly string[] | null
  /** The decision log, by id and state kind; `null` when it could not be read. */
  decisions: ReadonlyArray<{ id: string; state: string }> | null
}

interface TeamKept {
  mine: ReadonlySet<string>
  waiting: ReadonlySet<string>
  blocked: ReadonlySet<string>
}

/** The last good answer of each source. `undefined` is a source not read well yet. */
export interface EngineState {
  teams: ReadonlyMap<string, TeamKept>
  devices: ReadonlySet<string> | undefined
  decisions: ReadonlyMap<string, string> | undefined
}

export const START: EngineState = { teams: new Map(), devices: undefined, decisions: undefined }

const KINDS: readonly EventKind[] = ['request', 'participant', 'blocked', 'device', 'decision']

/** Order by Unicode code point, not by UTF-16 unit, so that every client orders the same way. */
function compareCodePoints(a: string, b: string): number {
  const left = Array.from(a, (character) => character.codePointAt(0) ?? 0 /* stand-in-ok(index): a character code, to order two strings */)
  const right = Array.from(b, (character) => character.codePointAt(0) ?? 0 /* stand-in-ok(index): a character code, to order two strings */)
  for (let index = 0; index < Math.min(left.length, right.length); index += 1) {
    const difference = (left[index] ?? 0 /* stand-in-ok(index): a character code, to order two strings */) - (right[index] ?? 0)
    if (difference !== 0) return difference
  }
  return left.length - right.length
}

const fresh = (now: readonly string[], before: ReadonlySet<string>): string[] => now.filter((one) => !before.has(one))

function teamEvents(state: EngineState, reading: TeamReading): { kept: TeamKept; events: NoticeEvent[] } {
  const where = `${reading.org}/${reading.team}`
  const kept: TeamKept = { mine: new Set(reading.mine), waiting: new Set(reading.waitingOnViewer), blocked: new Set(reading.blocked.map((item) => item.ref)) }
  const before = state.teams.get(where)
  if (before === undefined) return { kept, events: [] }
  return {
    kept,
    events: [
      ...fresh(reading.mine, before.mine).map((id): NoticeEvent => ({ kind: 'request', key: `request:${where}/${id}` })),
      ...fresh(reading.waitingOnViewer, before.waiting).map((id): NoticeEvent => ({ kind: 'participant', key: `participant:${where}/${id}` })),
      ...fresh(reading.blocked.map((item) => item.ref), before.blocked).map((ref): NoticeEvent => ({ kind: 'blocked', key: `blocked:${where}/${ref}` })),
    ],
  }
}

function decisionEvents(before: ReadonlyMap<string, string>, log: NonNullable<Reading['decisions']>, own: Own): NoticeEvent[] {
  const sent = new Set(own.sentDecisionIds)
  return log.flatMap(({ id, state }): NoticeEvent[] => {
    const known = before.get(id)
    // A decision this client sent, that went through, is not news; one that is queued or refused is, and so is any change of state.
    const isNews = known === undefined ? state === 'queued' || state === 'refused' || !sent.has(id) : known !== state
    return isNews ? [{ kind: 'decision', key: `decision:${id}:${state}` }] : []
  })
}

/** The next state, and the events this reading makes, ordered by kind and then by key. */
export function advance(state: EngineState, reading: Reading, own: Own): { state: EngineState; events: NoticeEvent[] } {
  const teams = new Map(state.teams)
  const events: NoticeEvent[] = []
  for (const team of reading.teams) {
    if (!team.ok) continue
    const { kept, events: made } = teamEvents(state, team)
    teams.set(`${team.org}/${team.team}`, kept)
    events.push(...made)
  }
  let devices = state.devices
  if (reading.devices !== null) {
    if (state.devices !== undefined && own.canApprove) {
      events.push(...fresh(reading.devices, state.devices).filter((key) => key !== own.deviceKey).map((key): NoticeEvent => ({ kind: 'device', key: `device:${key}` })))
    }
    devices = new Set(reading.devices)
  }
  let decisions = state.decisions
  if (reading.decisions !== null) {
    if (state.decisions !== undefined) events.push(...decisionEvents(state.decisions, reading.decisions, own))
    decisions = new Map(reading.decisions.map(({ id, state: kind }) => [id, kind]))
  }
  const ordered = events.toSorted((a, b) => KINDS.indexOf(a.kind) - KINDS.indexOf(b.kind) || compareCodePoints(a.key, b.key))
  return { state: { teams, devices, decisions }, events: ordered }
}
