// The Client API between the web client and its home node. These shapes are not generated from
// Rust yet; they are the web client's side of the calls and stay small on purpose.
import type { AnswerWith } from './teamview'

/** The signed-in person and the organizations and teams they can reach through their home node. */
export interface Me {
  account: { id: string; label: string }
  orgs: Array<{
    id: string
    label: string
    teams: Array<{ id: string; label: string }>
  }>
}

/** One decision: which request it answers (`answerWith` is copied from the request) and what was decided. */
export interface DecisionAnswer {
  org: string
  team: string
  answerWith: AnswerWith
  choice?: string
  text: string
}

// ── Organization structure ──
// The subset of the org definition (`agency-proto` OrgDefinition) that the org chart reads.
// Keys and keyless-when-absent behavior follow the Rust type; keys, versions, and node URLs are left out.

export interface OrgDepartment {
  id: string
  label: string
  parent?: string
  /** A person id from `people`. */
  head?: string
}

export interface OrgPerson {
  id: string
  label: string
  department?: string
  role: 'member' | 'answerer'
}

export interface OrgTeam {
  id: string
  label: string
  department?: string
}

export interface OrgDefinition {
  org: string
  label: string
  version: number
  departments: OrgDepartment[]
  people: OrgPerson[]
  teams: OrgTeam[]
}

// ── Costs ──
// Funding belongs to the account, not to a team runtime, so the node serves it directly.

export type WindowStatus = 'waste' | 'ok' | 'overrun'

/** One usage window of a subscription, with the node's forecast for when it resets. */
export interface FundingWindow {
  /** Percent of the window used so far. */
  utilization: number
  resetsAt: string
  /** Percent expected at the reset if the current pace holds. */
  projectedAtReset: number
  /** When the limit is expected to be hit, if before the reset. */
  hitAt?: string
  status: WindowStatus
}

export interface FundingSource {
  id: string
  label: string
  kind: 'subscription' | 'apiKey'
  sevenDay?: FundingWindow
  fiveHour?: FundingWindow
  /** The teams that draw on this source. */
  teams: Array<{ org: string; team: string }>
}

export interface TokenCounts {
  input: number
  output: number
  cacheRead: number
  cacheWrite: number
}

/** Token use of one team over the last `days` days, priced as if billed by the API. */
export interface UsageReport {
  org: string
  team: string
  days: number
  generatedAt: string
  /** Oldest day first. `costUsd` is keyed by the team runtime's person name. */
  byDay: Array<{ date: string; costUsd: Record<string, number> }>
  byPerson: Array<{ person: string; costUsd: number; tokens: TokenCounts }>
}
