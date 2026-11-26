import type { DecisionLog, DeviceList } from '../api/clientApi'
import type { Team } from '../api/teams'
import type { TeamState } from '../api/queries'
import type { Reading, TeamReading } from './engine'

/** What the engine reads of a team the node answered for. */
export const teamReadingOf = ({ view, mine, waitingOnViewer }: Team): TeamReading => ({
  org: view.org,
  team: view.team,
  ok: true,
  mine: [...mine],
  waitingOnViewer: [...waitingOnViewer],
  blocked: view.work.flatMap((item) => (item.blocked === undefined ? [] : [{ ref: item.ref, reason: item.blocked.reason }])),
})

/** A team the node could not answer for, or has not answered for yet: nothing is read of it. */
export const unreadTeam = (org: string, team: string): TeamReading => ({ org, team, ok: false, mine: [], waitingOnViewer: [], blocked: [] })

/** The teams of the account, each as read, failed, or still loading (which counts as not read). */
export function teamReadings(orgs: ReadonlyArray<{ org: { id: string }; teams: ReadonlyArray<{ ref: { id: string }; state: TeamState }> }>): TeamReading[] {
  return orgs.flatMap(({ org, teams }) => teams.map(({ ref, state }) => (state.status === 'ready' ? teamReadingOf(state.team) : unreadTeam(org.id, ref.id))))
}

/** The public keys of the devices that wait to be added. */
export const deviceKeysOf = (list: DeviceList | undefined): string[] | null => (list === undefined ? null : list.requests.map((request) => request.publicKey))

/** The decision log by id and state. */
export const decisionStatesOf = (log: DecisionLog | undefined): Reading['decisions'] => (log === undefined ? null : log.entries.map((entry) => ({ id: entry.id, state: entry.state.kind })))
