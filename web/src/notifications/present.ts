import type { DecisionLog, DeviceList } from '../api/clientApi'
import type { Team } from '../api/teams'
import type { Messages } from '../i18n/en'
import type { NoticeEvent } from './engine'

export interface Lookups {
  teams: readonly Team[]
  devices: DeviceList | undefined
  decisions: DecisionLog | undefined
}

/** Where a notice leads: the page that shows what it is about (and, for one request, that request on the desk). */
export type NoticeTarget = { to: '/desk'; item?: string } | { to: '/people' } | { to: '/devices' }

export interface Notice {
  message: string
  tone: 'info' | 'error'
  target: NoticeTarget
  /**
   * Names what a notice is about, so that a browser replaces one notification with another about the same thing
   * (the page's and a worker's, for one event): `request/<org>/<team>/<id>`, `device`, `decision/<id>`. Only a
   * notice of one event, of a kind a worker is also told of, has one.
   */
  tag?: string
}

/** The most notices one reading shows; the rest are counted in the last. */
const MOST = 3

/** `org/team/rest` of a key, split at the first two slashes: what follows is an id or a ref and may hold any character. */
function placeOf(rest: string): { org: string; team: string; item: string } {
  const first = rest.indexOf('/')
  const second = rest.indexOf('/', first + 1)
  return { org: rest.slice(0, first), team: rest.slice(first + 1, second), item: rest.slice(second + 1) }
}

const teamOf = (lookups: Lookups, org: string, team: string): Team | undefined => lookups.teams.find((one) => one.view.org === org && one.view.team === team)

const keysOf = (events: readonly NoticeEvent[], kind: NoticeEvent['kind'], prefix: string): string[] => events.filter((event) => event.kind === kind).map((event) => event.key.slice(prefix.length))

/**
 * What to tell the viewer about some events, in their words, in the page only: one notice for each kind (and for
 * each state of a decision), the first of a kind named, the others counted. A refused decision is an error. The
 * devices screen lists devices by itself, so it asks for none.
 */
export function noticesFor(events: readonly NoticeEvent[], lookups: Lookups, t: Messages, { onDevices }: { onDevices: boolean }): Notice[] {
  const text = t.notices
  const found: Notice[] = []

  const requests = keysOf(events, 'request', 'request:')
  if (requests.length > 0) {
    const { org, team, item } = placeOf(requests[0] ?? '' /* stand-in-ok(index): the first of a list that is not empty */)
    const owner = teamOf(lookups, org, team)
    const title = owner?.view.requests.find((one) => one.id === item)?.title
    const target: NoticeTarget = requests.length === 1 ? { to: '/desk', item: `${org}:${team}:${item}` } : { to: '/desk' }
    found.push({ tone: 'info', target, ...(requests.length === 1 ? { tag: `request/${org}/${team}/${item}` } : {}), message: text.request(requests.length, title ?? item, owner?.view.label ?? team) })
  }

  const participants = keysOf(events, 'participant', 'participant:')
  if (participants.length > 0) {
    const { org, team, item } = placeOf(participants[0] ?? '' /* stand-in-ok(index): the first of a list that is not empty */)
    const label = teamOf(lookups, org, team)?.view.people.find((one) => one.id === item)?.label
    found.push({ tone: 'info', target: { to: '/people' }, message: text.participant(participants.length, label ?? item) })
  }

  const blocked = keysOf(events, 'blocked', 'blocked:')
  if (blocked.length > 0) {
    const { org, team, item } = placeOf(blocked[0] ?? '' /* stand-in-ok(index): the first of a list that is not empty */)
    const owner = teamOf(lookups, org, team)
    const work = owner?.view.work.find((one) => one.ref === item)
    found.push({ tone: 'info', target: { to: '/people' }, message: text.blocked(blocked.length, work?.title ?? item, owner?.view.label ?? team, work?.blocked === undefined ? undefined : t.people.blockedReasons[work.blocked.reason]) })
  }

  const devices = onDevices ? [] : keysOf(events, 'device', 'device:')
  if (devices.length > 0) {
    const label = lookups.devices?.requests.find((one) => one.publicKey === devices[0])?.label
    found.push({ tone: 'info', target: { to: '/devices' }, ...(devices.length === 1 ? { tag: 'device' } : {}), message: text.device(devices.length, label ?? text.aDevice) })
  }

  const decided = events.filter((event) => event.kind === 'decision').map((event) => ({ id: event.key.slice('decision:'.length, event.key.lastIndexOf(':')), state: event.key.slice(event.key.lastIndexOf(':') + 1) }))
  for (const state of ['refused', 'queued', 'applied', 'delivered']) {
    const some = decided.filter((one) => one.state === state)
    if (some.length === 0) continue
    const entry = lookups.decisions?.entries.find((one) => one.id === some[0]?.id)
    const reason = entry?.state.kind === 'refused' ? entry.state.message : undefined
    found.push({ tone: state === 'refused' ? 'error' : 'info', target: { to: '/desk' }, ...(some.length === 1 ? { tag: `decision/${some[0]?.id}` } : {}), message: text.decision(state, some.length, entry?.title ?? '' /* stand-in-ok(optional): a decision logged without a title has none to name, in a notice of something that has just happened */, reason) })
  }

  return found.length <= MOST ? found : [...found.slice(0, MOST - 1), { tone: 'info', target: { to: '/desk' }, message: text.more(found.length - (MOST - 1)) }]
}
