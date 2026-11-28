// What the page keeps of how long its calls took, and when it reports it: every minute while it is shown, and when it is hidden or closed
// (so that the last minute is not lost). Only the owner's page reports: a hosted member's page keeps nothing.
import type { Sample } from './samples'
import { createSampleQueue } from './samples'
import { isReportable } from './reportable'
import { createReporter, REPORT_EVERY_MS, type Reporter } from './reporter'

export interface Telemetry {
  /** Told by the API client of each call; keeps it for the next report when its route is one the node takes. */
  record: (sample: Sample) => void
  /** This viewer is not the owner: what is kept is forgotten, and nothing more is kept or sent by this page. */
  forbid: () => void
  /** Begins reporting through `report`; returns what ends it. */
  start: (report: (samples: readonly Sample[]) => Promise<unknown>) => () => void
}

export function createTelemetry(): Telemetry {
  const queue = createSampleQueue()
  let isForbidden = false
  let reporter: Reporter | undefined
  return {
    record: (sample) => {
      if (!isForbidden && isReportable(sample.route)) queue.add(sample)
    },
    forbid: () => {
      isForbidden = true
      queue.clear()
    },
    start: (report) => {
      reporter ??= createReporter({ queue, report })
      const active = reporter
      const flush = (): void => void active.flush()
      const flushWhenHidden = (): void => {
        if (document.visibilityState === 'hidden') flush()
      }
      const timer = setInterval(flush, REPORT_EVERY_MS)
      document.addEventListener('visibilitychange', flushWhenHidden)
      window.addEventListener('pagehide', flush)
      return () => {
        clearInterval(timer)
        document.removeEventListener('visibilitychange', flushWhenHidden)
        window.removeEventListener('pagehide', flush)
      }
    },
  }
}
