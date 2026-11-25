import type { Team } from '../api/teams'
import type { BlockedReason } from '../api/teamview'
import type { TeamState } from '../api/queries'

/** What a team needs from the viewer, most pressing first: waiting on them, stuck, nobody working, or fine. */
export type Attention = 'mine' | 'stuck' | 'idle' | 'active'

export interface TeamSummary {
  attention: Attention
  /** Requests that are the viewer's to decide: the node's list, never worked out here. */
  mine: number
  /** The team's participants without the viewer, and how many of them are working. */
  participants: number
  working: number
  /** Blocked work items, by the reason the team's runtime gives. */
  blocked: ReadonlyArray<{ reason: BlockedReason; count: number }>
  blockedCount: number
  /** What the participants who are not working say they are doing, as the team reports it; at most two lines. */
  quiet: readonly string[]
}

const REASONS: readonly BlockedReason[] = ['needsDecision', 'external', 'waitingOnPrevious', 'timeOrEquipment']
const QUIET_LINES = 2

export function summarize({ view, viewer, mine }: Team): TeamSummary {
  const participants = view.people.filter((person) => person.id !== viewer)
  const working = participants.filter((person) => person.state === 'working').length
  const blocked = REASONS.map((reason) => ({ reason, count: view.work.filter((item) => item.blocked?.reason === reason).length })).filter((entry) => entry.count > 0)
  const blockedCount = blocked.reduce((sum, entry) => sum + entry.count, 0)
  const quiet = [...new Set(participants.filter((person) => person.state !== 'working' && person.status.trim() !== '').map((person) => person.status.trim()))].slice(0, QUIET_LINES)
  const attention: Attention = mine.size > 0 ? 'mine' : blockedCount > 0 ? 'stuck' : participants.length > 0 && working === 0 ? 'idle' : 'active'
  return { attention, mine: mine.size, participants: participants.length, working, blocked, blockedCount, quiet }
}

const RANK: Record<Attention, number> = { mine: 0, stuck: 1, idle: 3, active: 4 }
/** A team that cannot be read comes after the stuck ones and before the idle ones; one still loading comes last. */
const FAILED_RANK = 2
const LOADING_RANK = 5

export interface Entry {
  label: string
  state: TeamState
}

export function rankOf(state: TeamState): number {
  if (state.status === 'failed') return FAILED_RANK
  return state.status === 'loading' ? LOADING_RANK : RANK[summarize(state.team).attention]
}

/** Within one rank, the team with more waiting on the viewer or more blocked comes first, then by name. */
function weightOf(state: TeamState): number {
  if (state.status !== 'ready') return 0
  const summary = summarize(state.team)
  return summary.attention === 'mine' ? summary.mine : summary.blockedCount
}

export const orderTeams = <T extends Entry>(teams: readonly T[]): T[] =>
  teams.toSorted((a, b) => rankOf(a.state) - rankOf(b.state) || weightOf(b.state) - weightOf(a.state) || a.label.localeCompare(b.label))

/** An org comes where its most pressing team does. */
export const orgRank = (teams: readonly Entry[]): number => Math.min(LOADING_RANK, ...teams.map((team) => rankOf(team.state)))
