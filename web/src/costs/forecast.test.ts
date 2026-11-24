import type { FundingWindowReading } from '../api/clientApi'
import { forecastWindow, WINDOW_MS } from './forecast'

const HOUR = 3_600_000
const NOW = Date.parse('2026-11-25T12:00:00Z')

/** A window that is `elapsedHours` into its length, with the given use so far. */
const reading = (name: FundingWindowReading['name'], elapsed: number, utilization: number): FundingWindowReading => ({
  name,
  utilization,
  resetsAt: new Date(NOW + WINDOW_MS[name] - elapsed).toISOString(),
})

describe('forecastWindow', () => {
  it('extrapolates the pace so far to the reset', () => {
    const forecast = forecastWindow(reading('fiveHour', 2.5 * HOUR, 40), NOW)
    expect(forecast?.projectedAtReset).toBeCloseTo(80)
    expect(forecast?.status).toBe('ok')
    expect(forecast?.hitAt).toBeUndefined()
  })

  it('says when the limit is reached if the pace would overrun the window', () => {
    const forecast = forecastWindow(reading('fiveHour', 3 * HOUR, 74), NOW)
    expect(forecast?.status).toBe('overrun')
    expect(forecast?.projectedAtReset).toBeCloseTo(123.3, 1)
    // 74% took 3 hours, so 100% comes at about 4.05 hours: roughly 1.05 hours from now.
    expect(Date.parse(forecast?.hitAt ?? '') - NOW).toBeCloseTo(1.054 * HOUR, -4)
  })

  it('calls a window that will end well short of full unused', () => {
    expect(forecastWindow(reading('sevenDay', 4 * 24 * HOUR, 30), NOW)?.status).toBe('waste')
  })

  it('treats a full window as run out, with nothing left to predict', () => {
    const forecast = forecastWindow(reading('fiveHour', HOUR, 100), NOW)
    expect(forecast?.status).toBe('overrun')
    expect(forecast?.hitAt).toBeUndefined()
  })

  it('does not extrapolate from the first minutes of a window', () => {
    const forecast = forecastWindow(reading('sevenDay', HOUR, 5), NOW)
    expect(forecast?.projectedAtReset).toBe(5)
    expect(forecast?.status).toBe('ok')
  })

  it('gives no forecast once the reset has passed, since the reading is out of date', () => {
    expect(forecastWindow({ name: 'fiveHour', utilization: 50, resetsAt: new Date(NOW - 1).toISOString() }, NOW)).toBeUndefined()
  })
})
