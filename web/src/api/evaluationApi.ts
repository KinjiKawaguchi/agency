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
