// The Client API between the web client and its home node (docs/protocol.md, "Client API").
// Its types are generated from the Rust types in `bindings/` and are never edited by hand. The only
// types defined here are the client's own: the arguments of a decision call, and token use, which the
// node does not serve yet (agency#22).
import type { AnswerWith } from './teamview'

export type { AccountRef } from '../../../bindings/AccountRef'
export type { DecisionEntry } from '../../../bindings/DecisionEntry'
export type { DecisionInput } from '../../../bindings/DecisionInput'
export type { DecisionLog } from '../../../bindings/DecisionLog'
export type { DecisionOutcome } from '../../../bindings/DecisionOutcome'
export type { DecisionResult } from '../../../bindings/DecisionResult'
export type { DecisionState } from '../../../bindings/DecisionState'
export type { Department } from '../../../bindings/Department'
export type { Funding } from '../../../bindings/Funding'
export type { FundingKind } from '../../../bindings/FundingKind'
export type { Me } from '../../../bindings/Me'
export type { OrgDefinition } from '../../../bindings/OrgDefinition'
export type { OrgRef } from '../../../bindings/OrgRef'
export type { Person } from '../../../bindings/Person'
export type { SnapshotSource } from '../../../bindings/SnapshotSource'
export type { TeamLocation } from '../../../bindings/TeamLocation'
export type { TeamPlace } from '../../../bindings/TeamPlace'
export type { TeamRef } from '../../../bindings/TeamRef'
export type { TeamSnapshot } from '../../../bindings/TeamSnapshot'
export type { UsageWindow } from '../../../bindings/UsageWindow'
export type { UsageWindowName } from '../../../bindings/UsageWindowName'

/** One decision: which request it answers (`answerWith` is copied from the request) and what was decided. */
export interface DecisionAnswer {
  org: string
  team: string
  answerWith: AnswerWith
  /** The request's title. The request leaves the team's view once answered, so the node keeps the title in its log. */
  title: string
  choice?: string
  text: string
}

// ── Token use (not in the node's contract yet) ──

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
