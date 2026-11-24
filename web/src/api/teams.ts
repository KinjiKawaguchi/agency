import type { TeamSnapshot, TeamSource } from './clientApi'
import type { TeamView } from './teamview'

/** A team as the app uses it: what the team reported, and who the viewer is in it. */
export interface Team {
  view: TeamView
  /** The team runtime's name for the person the viewer answers for. */
  viewer: string
  source: TeamSource
}

/**
 * The name the viewer goes by in a team. The node reports it; until it does, the viewer is taken to be
 * the team's owner. This is the one place that guess lives.
 */
export const viewerNameOf = (snapshot: TeamSnapshot): string => snapshot.viewer ?? 'owner'

export const toTeam = (snapshot: TeamSnapshot): Team => ({ view: snapshot.view, viewer: viewerNameOf(snapshot), source: snapshot.source })

export const teamKey = (team: Team): string => `${team.view.org}/${team.view.team}`
