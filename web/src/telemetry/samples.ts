// How long the page's own calls to its home node took, as it reports them: a route template (never a path with an org or a team in it),
// the class of the answer, and the milliseconds a person waited. Kept until the next report; only the newest are kept.

export type SampleStatus = '2xx' | '3xx' | '4xx' | '5xx' | 'network'

/** One call: the node's route template (`GET /api/orgs/{org}/teams/{team}/view`), how it ended, and how long it took in whole milliseconds. */
export interface Sample {
  route: string
  status: SampleStatus
  ms: number
}

/** What the node takes in one report. */
export const MAX_SAMPLES_PER_REPORT = 200

/** The last bucket of the node's histogram starts after this: a longer call is all the same to it. */
const LONGEST_MS = 30_001

/** The class of an HTTP status; a status outside 200 to 599 is not an answer of the node. */
export function statusOf(httpStatus: number): SampleStatus | undefined {
  if (httpStatus >= 200 && httpStatus < 300) return '2xx'
  if (httpStatus >= 300 && httpStatus < 400) return '3xx'
  if (httpStatus >= 400 && httpStatus < 500) return '4xx'
  if (httpStatus >= 500 && httpStatus < 600) return '5xx'
  return undefined
}

/** Whole milliseconds, never negative, and no more than the node tells apart. */
export const wholeMs = (ms: number): number => Math.min(LONGEST_MS, Math.max(0, Math.round(ms)))

/** The samples waiting for the next report: the newest `MAX_SAMPLES_PER_REPORT`, oldest first; the older ones are dropped, never sent in a second request. */
export interface SampleQueue {
  add: (sample: Sample) => void
  /** Takes everything that waits, oldest first, and leaves nothing. */
  take: () => Sample[]
  /** Puts samples that were taken but not taken in back in front of the newer ones, within the same limit: the newest are kept. */
  putBack: (samples: readonly Sample[]) => void
  size: () => number
  clear: () => void
}

export function createSampleQueue(limit = MAX_SAMPLES_PER_REPORT): SampleQueue {
  let waiting: readonly Sample[] = []
  return {
    add: (sample) => {
      waiting = [...waiting, sample].slice(-limit)
    },
    take: () => {
      const taken = [...waiting]
      waiting = []
      return taken
    },
    putBack: (samples) => {
      waiting = [...samples, ...waiting].slice(-limit)
    },
    size: () => waiting.length,
    clear: () => {
      waiting = []
    },
  }
}
