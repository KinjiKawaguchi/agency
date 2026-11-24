import type { FundingWindowReading } from '../api/clientApi'

export type WindowStatus = 'waste' | 'ok' | 'overrun'

const HOUR = 3_600_000

/** How long each window is, which the node does not report. */
export const WINDOW_MS: Record<FundingWindowReading['name'], number> = { fiveHour: 5 * HOUR, sevenDay: 7 * 24 * HOUR }

/** Below this share of the window there is too little to extrapolate from. */
const MIN_ELAPSED_SHARE = 0.02

/** A window projected to end below this percent counts as going unused. */
export const WASTE_BELOW_PERCENT = 80

export interface WindowForecast {
  utilization: number
  resetsAt: string
  /** Percent expected at the reset if the pace so far holds. */
  projectedAtReset: number
  /** When the limit is reached, if that happens before the reset. */
  hitAt?: string
  status: WindowStatus
}

/**
 * Projects a window to its reset from the pace so far. Returns `undefined` when the reset has already
 * passed, which means the reading is out of date and says nothing about the current window.
 */
export function forecastWindow(reading: FundingWindowReading, now: number): WindowForecast | undefined {
  const resetsAt = Date.parse(reading.resetsAt)
  const length = WINDOW_MS[reading.name]
  const remaining = resetsAt - now
  if (remaining <= 0) return undefined

  const elapsed = Math.min(length, Math.max(0, length - remaining))
  const hasPace = elapsed >= length * MIN_ELAPSED_SHARE && reading.utilization > 0
  const projectedAtReset = hasPace ? (reading.utilization * length) / elapsed : reading.utilization
  const isFull = reading.utilization >= 100
  const willRunOut = isFull || projectedAtReset > 100
  const hitAt = !isFull && willRunOut ? new Date(resetsAt - length + (elapsed * 100) / reading.utilization).toISOString() : undefined
  const status: WindowStatus = willRunOut ? 'overrun' : projectedAtReset < WASTE_BELOW_PERCENT && elapsed >= length * MIN_ELAPSED_SHARE ? 'waste' : 'ok'
  return { utilization: reading.utilization, resetsAt: reading.resetsAt, projectedAtReset, ...(hitAt === undefined ? {} : { hitAt }), status }
}
