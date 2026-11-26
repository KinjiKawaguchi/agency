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
}

export const toTeam = (snapshot: TeamSnapshot): Team => ({
  view: snapshot.view,
  viewer: snapshot.viewer,
  source: snapshot.source,
  mine: new Set(snapshot.mine),
  waitingOnViewer: new Set(snapshot.waitingOnViewer ?? [] /* stand-in-ok(optional): the node leaves the list out when it is empty, in an answer that has come */),
})

export const teamKey = (team: Team): string => `${team.view.org}/${team.view.team}`
