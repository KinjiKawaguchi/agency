import type { PersonStatus, RequestKind, TeamRequest, TeamView } from '../api/teamview'
import { presenceOf } from '../people/state'
import { viewerNameOf } from '../people/viewer'

export type GroupId = RequestKind | 'others'

/** Display order of the groups in the pile. */
export const GROUP_ORDER: readonly GroupId[] = ['proposal', 'question', 'pendingDecision', 'others']

export interface DeskItem {
  /** Unique across orgs and teams. Also the key of the item's draft. */
  key: string
  group: GroupId
  request: TeamRequest
  view: TeamView
  /** Absent when the request comes from something that is not a person on the team, such as the board. */
  drafter: PersonStatus | undefined
  /** The person it is addressed to. Absent `to` means the owner. */
  decider: PersonStatus | undefined
  /** True when the viewer, as the team's owner, is the one who has to decide. */
  isMine: boolean
}

/** Only the owner a request is addressed to can decide it. */
export const canDecide = (item: DeskItem): boolean => item.isMine

export const personOf = (view: TeamView, name: string): PersonStatus | undefined => view.people.find((person) => person.id === name)

export const itemKey = (view: TeamView, request: TeamRequest): string => `${view.org}:${view.team}:${request.id}`

function toItem(view: TeamView, request: TeamRequest): DeskItem {
  const viewerName = viewerNameOf(view)
  const to = request.to ?? viewerName
  const isMine = to === viewerName
  return {
    key: itemKey(view, request),
    group: isMine ? request.kind : 'others',
    request,
    view,
    drafter: personOf(view, request.from),
    decider: personOf(view, to),
    isMine,
  }
}

/** The people (not work refs) that are stopped until the request is decided. */
export const blockedPeople = (item: DeskItem): PersonStatus[] =>
  item.request.blocks.flatMap((name) => {
    const person = personOf(item.view, name)
    return person === undefined ? [] : [person]
  })

function compareItems(a: DeskItem, b: DeskItem): number {
  return (
    GROUP_ORDER.indexOf(a.group) - GROUP_ORDER.indexOf(b.group) ||
    blockedPeople(b).length - blockedPeople(a).length ||
    a.request.createdAt.localeCompare(b.request.createdAt)
  )
}

/** Requests from every team, grouped, with the ones that stop the most people first and oldest first within that. */
export function openItems(views: readonly TeamView[]): DeskItem[] {
  return views.flatMap((view) => view.requests.map((request) => toItem(view, request))).toSorted(compareItems)
}

export interface Situation {
  waitingOnYou: number
  working: number
  total: number
  mine: number
}

/** The numbers behind the one-line summary. The owner is the viewer and is not counted among the people. */
export function situationOf(views: readonly TeamView[], items: readonly DeskItem[]): Situation {
  const others = views.flatMap((view) => view.people.filter((person) => person.id !== viewerNameOf(view)).map((person) => ({ person, view })))
  return {
    waitingOnYou: others.filter(({ person, view }) => presenceOf(person, viewerNameOf(view)) === 'needs-you').length,
    working: others.filter(({ person }) => person.state === 'working').length,
    total: others.length,
    mine: items.filter((item) => item.isMine).length,
  }
}

/** The key that should be open: the current one if it is still on the desk, else the first. */
export function resolveSelection(items: readonly DeskItem[], selected: string | undefined): string | undefined {
  return items.some((item) => item.key === selected) ? selected : items[0]?.key
}

/** The key `step` places away from `from`, stopping at the ends. */
export function stepSelection(items: readonly DeskItem[], from: string | undefined, step: 1 | -1): string | undefined {
  if (items.length === 0) return undefined
  const index = items.findIndex((item) => item.key === from)
  const next = Math.max(0, Math.min(items.length - 1, (index === -1 ? 0 : index) + step))
  return items[next]?.key
}
