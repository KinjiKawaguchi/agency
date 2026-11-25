import type { UsageWindow } from '../api/clientApi'

export type WindowStatus = 'waste' | 'ok' | 'overrun'

const HOUR = 3_600_000

/** Below this share of the window there is too little to extrapolate from. */
const MIN_ELAPSED_SHARE = 0.02

/** A window projected to end below this percent counts as going unused. */
export const WASTE_BELOW_PERCENT = 85

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
 * Projects a window to its reset from the pace so far, using the window's own length. Returns `undefined`
 * when the reset has already passed, which means the reading is out of date and says nothing about the
 * current window, or when the length is not a positive number.
 */
export function forecastWindow(reading: UsageWindow, now: number): WindowForecast | undefined {
  const resetsAt = Date.parse(reading.resetsAt)
  const length = reading.lengthHours * HOUR
  const remaining = resetsAt - now
  if (remaining <= 0 || !(length > 0)) return undefined

  const elapsed = Math.min(length, Math.max(0, length - remaining))
  const hasPace = elapsed >= length * MIN_ELAPSED_SHARE && reading.utilization > 0
  const projectedAtReset = hasPace ? (reading.utilization * length) / elapsed : reading.utilization
  const isFull = reading.utilization >= 100
  const willRunOut = isFull || projectedAtReset > 100
  const hitAt = !isFull && willRunOut ? new Date(resetsAt - length + (elapsed * 100) / reading.utilization).toISOString() : undefined
  const status: WindowStatus = willRunOut ? 'overrun' : projectedAtReset < WASTE_BELOW_PERCENT && elapsed >= length * MIN_ELAPSED_SHARE ? 'waste' : 'ok'
  return { utilization: reading.utilization, resetsAt: reading.resetsAt, projectedAtReset, ...(hitAt === undefined ? {} : { hitAt }), status }
}
