// The Client API between the web client and its home node. These shapes follow what the node will
// serve (agency#4) and are replaced by the generated bindings when they land. Token use
// (`UsageReport`) is not part of that contract yet and is served by the mock node only.
import type { AnswerWith, TeamView } from './teamview'

// ── Who the viewer is, and what they can reach ──

export interface FundingWindowReading {
  name: 'fiveHour' | 'sevenDay'
  /** Percent of the window used so far. */
  utilization: number
  resetsAt: string
}

/** A funding source, as the node last read it. Forecasts are worked out by the client. */
export interface FundingSource {
  id: string
  label: string
  kind: 'subscription' | 'apiKey'
  windows: FundingWindowReading[]
  /** When the node last read the usage. */
  readAt?: string
}

export interface TeamRef {
  id: string
  label: string
  location: 'local' | 'remote'
  /** The viewer's runtime name in this team. */
  viewer?: string
}

export interface Me {
  account: { id: string; label: string; fingerprint: string }
  orgs: Array<{ id: string; label: string; isRoot: boolean; teams: TeamRef[] }>
  funding: FundingSource[]
}

// ── A team ──

export type TeamSource =
  | { kind: 'local' }
  /** A copy read from the team's own node. `error` says why the last read failed. */
  | { kind: 'replica'; fetchedAt?: string; triedAt: string; error?: string }

export interface TeamSnapshot {
  view: TeamView
  /** The viewer's runtime name in this team, when the node knows it. */
  viewer?: string
  source: TeamSource
}

// ── Deciding ──

/** One decision: which request it answers (`answerWith` is copied from the request) and what was decided. */
export interface DecisionAnswer {
  org: string
  team: string
  answerWith: AnswerWith
  choice?: string
  text: string
}

export interface DecisionResult {
  /** `applied`: the team has it. `delivered`: sent on to the team's node. `queued`: kept until that node is reachable. */
  outcome: 'applied' | 'delivered' | 'queued'
  message?: string
}

// ── Organization structure ──
// The part of the org definition (`agency-proto` OrgDefinition) that the org chart reads.
// Optional fields are left out when absent, as in the Rust type; keys, versions, and node URLs are not needed here.

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
  /** The account that signs for this person. */
  account: string
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

// ── Token use ──

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
