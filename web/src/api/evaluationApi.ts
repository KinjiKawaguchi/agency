// The evaluation and effort routes of the Client API: how fast a team's merged pull requests ship, and where
// its effort goes. The node works the numbers out (docs/protocol.md, "Evaluation"). The types are generated from the
// Rust types in `bindings/` and are never edited by hand; only the client's own are defined here: the names of the
// pull request and work types as lists, and the team's plan, which the org definition does not carry yet.
import type { MergedCounts } from '../../../bindings/MergedCounts'
import type { WorkType } from '../../../bindings/WorkType'

export type { ByWorkType } from '../../../bindings/ByWorkType'
export type { DayEffort } from '../../../bindings/DayEffort'
export type { DayEvaluation } from '../../../bindings/DayEvaluation'
export type { Effort } from '../../../bindings/Effort'
export type { Evaluation } from '../../../bindings/Evaluation'
export type { EvaluationSummary } from '../../../bindings/EvaluationSummary'
export type { MergedCounts, WorkType }
export type { ParticipantEvaluation } from '../../../bindings/ParticipantEvaluation'
export type { RepositoryEvaluation } from '../../../bindings/RepositoryEvaluation'
export type { TeamEffort } from '../../../bindings/TeamEffort'
export type { TeamEvaluation } from '../../../bindings/TeamEvaluation'
export type { TeamUnavailable } from '../../../bindings/TeamUnavailable'
export type { TypeUsage } from '../../../bindings/TypeUsage'

/** The Conventional Commits type of a pull request's title, as the node counts them; anything else is `other`. */
export type PullRequestType = keyof MergedCounts

/** The types in the order they are listed and stacked. */
export const PULL_REQUEST_TYPES: readonly PullRequestType[] = ['feat', 'fix', 'refactor', 'docs', 'test', 'chore', 'other']

/** The work types, and the work nothing could classify, which the screen shows as it is. */
export type EffortType = WorkType | 'unclassified'

export const WORK_TYPES: readonly WorkType[] = ['newFeature', 'improvement', 'maintenance', 'debt', 'incident']

/** The work types in the order they stack, with the work no one could classify last. */
export const EFFORT_TYPES: readonly EffortType[] = [...WORK_TYPES, 'unclassified']

/**
 * What the org intends for a team: the share of its effort for each work type (whole numbers that sum to
 * 100) and its budget for a week, in whole US dollars at API prices. It is `teams[].plan` of the signed org
 * definition; AgencyCore owns the protocol change, and until its types are generated the client reads the
 * definition as `unknown` and checks the shape (`planOf`).
 */
export interface TeamPlan {
  allocation: Record<WorkType, number>
  weeklyBudgetUsd: number
}
