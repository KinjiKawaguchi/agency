// Sends the waiting samples to the node, one report at a time, and does what the node's answer says: a report it refuses as it is made
// (400, 413) is not sent again; a node that has no such route or does not take this viewer's reports (404, 405, 403) is not asked again
// by this page; anything else (a busy node, no answer, a sign-in that ended) is tried again after a pause, with the samples kept.
import { ApiError } from '../api/client'
import type { Sample, SampleQueue } from './samples'

/** How long a report waits for the next, while the page is shown. */
export const REPORT_EVERY_MS = 60_000

/** How long nothing is sent after a report that could not be taken. */
export const PAUSE_MS = 60_000

type Report = (samples: readonly Sample[]) => Promise<unknown>

/** What to do with a report that failed: forget its samples, stop for this page, or keep them for later. */
type Verdict = 'drop' | 'stop' | 'later'

function verdictOf(error: unknown): Verdict {
  if (!(error instanceof ApiError)) return 'later'
  if (error.status === 400 || error.status === 413) return 'drop'
  if (error.status === 403 || error.status === 404 || error.status === 405) return 'stop'
  return 'later'
}

export interface Reporter {
  /** Sends what waits, unless there is nothing, a report is on its way, the node asked for a pause, or this page has stopped. */
  flush: () => Promise<void>
  hasStopped: () => boolean
}

export function createReporter({ queue, report, now = Date.now }: { queue: SampleQueue; report: Report; now?: () => number }): Reporter {
  let hasStopped = false
  let isSending = false
  let pausedUntil = 0
  const settle = (error: unknown, samples: readonly Sample[]): void => {
    const verdict = verdictOf(error)
    if (verdict === 'stop') {
      hasStopped = true
      queue.clear()
    }
    if (verdict === 'later') {
      pausedUntil = now() + PAUSE_MS
      queue.putBack(samples)
    }
  }
  return {
    hasStopped: () => hasStopped,
    flush: async () => {
      if (hasStopped || isSending || now() < pausedUntil || queue.size() === 0) return
      const samples = queue.take()
      isSending = true
      try {
        await report(samples)
      } catch (error) {
        settle(error, samples)
      } finally {
        isSending = false
      }
    },
  }
}
