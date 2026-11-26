// The node's own measurements (`GET /api/metrics`, docs/protocol.md "Metrics"): how long the node takes to answer, and
// how long its own calls take. Owner only. `metricsAnswers.ts` reads an answer into these types before a screen sees it.

/** The fixed upper bounds, in milliseconds, of the first 14 buckets; the 15th holds what is slower. */
export const BUCKET_BOUNDS_MS: readonly number[] = [1, 2, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10_000, 30_000]
export const BUCKET_COUNT = BUCKET_BOUNDS_MS.length + 1

import type { ObjectiveResult, ObjectiveState } from './clientApi'

export type SeriesKind = 'serve' | 'call' | 'client'

export interface HourSeries {
  hour: string
  count: number
  errors: number
  /** The upper bound of the bucket; left out when the hour's p95 is beyond the last bucket. */
  p95Ms?: number
}

/** One kind, name, and status (and peer or client) of what the node measured. */
export interface LatencySeries {
  kind: SeriesKind
  /** A route template (`GET /api/orgs/{org}/teams/{team}/usage`) or a fixed phrase (`runtime view`). Never a path with values. */
  name: string
  status: string
  /** The route group the node says, or the first word of a call. */
  group: string
  peer?: string
  client?: string
  /** Whether the node answers this route from its own state alone; only a `serve` series can be. */
  isInNodeRead: boolean
  count: number
  errors: number
  sumMs: number
  /** `BUCKET_COUNT` counts that add up to `count`. */
  buckets: number[]
  p50Ms?: number
  p95Ms?: number
  p99Ms?: number
  /** Oldest first; empty for a range of more than 7 days, where the node gives the totals only. */
  byHour: HourSeries[]
}

/**
 * A response time the node's operator set for a route group, and whether it held, as the node says it: the generated
 * type. A state this client does not know is read as `unknown` (`metricsAnswers.ts`).
 */
export type Objective = ObjectiveResult
export type { ObjectiveState }

export interface Metrics {
  from: string
  until: string
  generatedAt: string
  /** The p95 an in-node read must keep, as the node says it; absent when the node says none. */
  budgetInNodeReadP95Ms?: number
  objectives: Objective[]
  series: LatencySeries[]
}
