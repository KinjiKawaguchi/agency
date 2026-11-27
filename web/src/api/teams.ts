import type { TeamSnapshot, SnapshotSource } from './clientApi'
import type { TeamView } from './teamview'

/** A team as the app uses it: what the team reported, and who the viewer is in it. */
export interface Team {
  view: TeamView
  /** The team runtime's name for the person the viewer answers for; absent when the team does not address the viewer. */
  viewer: string | undefined
  source: SnapshotSource
  /** Ids of the requests that are the viewer's to decide: the node's rule (docs/protocol.md, "Requests"), never worked out here. */
  mine: ReadonlySet<string>
  /** The ids of the team's people who wait on the viewer: the node's rule, never worked out here. */
  waitingOnViewer: ReadonlySet<string>
  /** For each request addressed to a seat or an external party that the viewer answers for: the label of whom it is for (the node's `relayedFor`). */
  relayedFor: ReadonlyMap<string, string>
}

export const toTeam = (snapshot: TeamSnapshot): Team => ({
  view: snapshot.view,
  viewer: snapshot.viewer,
  source: snapshot.source,
  mine: new Set(snapshot.mine),
  waitingOnViewer: new Set(snapshot.waitingOnViewer ?? [] /* stand-in-ok(optional): the node leaves the list out when it is empty, in an answer that has come */),
  // A label with nothing in it is no one to say a request is for.
  relayedFor: new Map(Object.entries(snapshot.relayedFor ?? {} /* stand-in-ok(optional): the node leaves it out when no request is for a seat the viewer answers for, in an answer that has come */).filter(([, label]) => label.trim() !== '')),
})

export const teamKey = (team: Team): string => `${team.view.org}/${team.view.team}`

/** The key of one request among all the teams': what the desk opens by, and what a decision made on it is kept under until the node has it. */
export const requestKey = (view: Pick<TeamView, 'org' | 'team'>, request: { id: string }): string => `${view.org}:${view.team}:${request.id}`

/** The team without the requests that were decided and are not yet done with (kept in the browser or queued at the node): they are not waiting on anyone any more. The same team when none of them is in it. */
export function withoutDecided(team: Team, decided: ReadonlySet<string>): Team {
  if (decided.size === 0 || !team.view.requests.some((request) => decided.has(requestKey(team.view, request)))) return team
  const left = team.view.requests.filter((request) => !decided.has(requestKey(team.view, request)))
  // What the node says is the viewer's to decide (`mine`) is cut to what is left: every count that reads it leaves the decided request too.
  return { ...team, view: { ...team.view, requests: left }, mine: new Set([...team.mine].filter((id) => left.some((request) => request.id === id))) }
}
