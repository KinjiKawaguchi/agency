// The evaluation route of the Client API: how fast a team's merged pull requests ship.
// The node computes the numbers (AgencyOps owns the route). These types stand in for the generated ones
// until the Rust types land in `bindings/`; then this file becomes a re-export, like `clientApi.ts`.

/** The Conventional Commits type of a pull request's title; anything else is `other`. */
export type PullRequestType = 'feat' | 'fix' | 'refactor' | 'docs' | 'test' | 'chore' | 'other'

/** Merged pull requests, counted by type. */
export type MergedCounts = Record<PullRequestType, number>

export interface DayEvaluation {
  /** `YYYY-MM-DD`, in the node's time zone. */
  date: string
  merged: MergedCounts
  /** Pull requests merged that day that revert another. */
  reverts: number
  /** Seconds main's CI was red that day; a span across midnight is split. */
  mainRedSeconds: number
  /** Tokens of that day, API-priced as in `TokenUsage`. */
  costUsd: number
  /** Pull request opened to merged, in seconds, one for each pull request merged that day. */
  prLeadSeconds: number[]
  /** Issue started to merged, in seconds, one for each merged pull request that has an issue. */
  issueLeadSeconds: number[]
}

/** The numbers an agent cites: the whole period, worked out by the node. */
export interface EvaluationSummary {
  merged: MergedCounts
  prLeadMedianSeconds: number | null
  /** The p90 is the nearest rank: the smallest value that at least 9 in 10 of the samples do not exceed. */
  prLeadP90Seconds: number | null
  issueLeadMedianSeconds: number | null
  issueLeadP90Seconds: number | null
  mainRedSeconds: number
  longestRedSeconds: number
  reverts: number
  costPerMergedPrUsd: number | null
}

export interface TeamEvaluation {
  team: string
  summary: EvaluationSummary
  /** Oldest first. Days with nothing in them may be left out. */
  byDay: DayEvaluation[]
}

/** `GET /api/orgs/<org>/evaluation?days=<1 to 90>` */
export interface Evaluation {
  org: string
  days: number
  generatedAt: string
  /**
   * The period's first and last calendar day, in the node's time zone, as `byDay[].date` writes them. The
   * node is asked to send them; until it does, the client takes the newest day of `byDay`.
   */
  from?: string
  until?: string
  teams: TeamEvaluation[]
}

// ── Where effort goes ──

/** What a piece of work is, as the board's "Work type" field or, failing that, the pull request's type says. */
export type WorkType = 'newFeature' | 'improvement' | 'maintenance' | 'debt' | 'incident'

/** The work types, and the work nothing could classify, which the screen shows as it is. */
export type EffortType = WorkType | 'unclassified'

export interface EffortTokens {
  input: number
  output: number
  cacheRead: number
  cacheWrite: number
}

export interface TypeUsage {
  /** API-priced, as in `TokenUsage`. */
  costUsd: number
  tokens: EffortTokens
}

export interface DayEffort {
  /** `YYYY-MM-DD`, in the node's time zone. */
  date: string
  costUsd: Record<EffortType, number>
}

export interface TeamEffort {
  team: string
  byType: Record<EffortType, TypeUsage>
  /** Oldest first. Days with nothing in them may be left out. */
  byDay: DayEffort[]
}

/** `GET /api/orgs/<org>/effort?days=<1 to 90>`: the cost and tokens of each team, by work type. */
export interface Effort {
  org: string
  days: number
  generatedAt: string
  /** The period's first and last calendar day in the node's time zone, as on `Evaluation`. */
  from?: string
  until?: string
  teams: TeamEffort[]
}

/**
 * What the org intends for a team: the share of its effort for each work type (whole numbers that sum to
 * 100) and its budget for a week, in whole US dollars at API prices. It is `teams[].plan` of the signed org
 * definition; AgencyCore owns the protocol change.
 */
export interface TeamPlan {
  allocation: Record<WorkType, number>
  weeklyBudgetUsd: number
}
