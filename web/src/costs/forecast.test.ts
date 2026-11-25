import type { UsageWindow } from '../api/clientApi'
import { forecastWindow } from './forecast'

const HOUR = 3_600_000
const NOW = Date.parse('2026-11-25T12:00:00Z')

const LENGTH_HOURS: Record<UsageWindow['name'], number> = { fiveHour: 5, sevenDay: 168 }

/** A window that is `elapsedHours` into its length, with the given use so far. */
const reading = (name: UsageWindow['name'], elapsed: number, utilization: number): UsageWindow => ({
  name,
  lengthHours: LENGTH_HOURS[name],
  utilization,
  resetsAt: new Date(NOW + LENGTH_HOURS[name] * HOUR - elapsed).toISOString(),
})

describe('forecastWindow', () => {
  it('extrapolates the pace so far to the reset', () => {
    const forecast = forecastWindow(reading('fiveHour', 2.5 * HOUR, 45), NOW)
    expect(forecast?.projectedAtReset).toBeCloseTo(90)
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

  it('calls a window unused only when it is projected to end under 85%', () => {
    // Half of the weekly window gone, so the projection is twice the use so far.
    const half = (utilization: number) => forecastWindow(reading('sevenDay', 3.5 * 24 * HOUR, utilization), NOW)
    expect(half(42)?.status).toBe('waste')
    expect(half(43)?.status).toBe('ok')
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
    expect(forecastWindow({ name: 'fiveHour', lengthHours: 5, utilization: 50, resetsAt: new Date(NOW - 1).toISOString() }, NOW)).toBeUndefined()
  })

  it('uses the length the node reports, not one it assumes for the name', () => {
    // Two days of a 10-day window gone at 20%: the pace says 100% at the reset.
    const custom = forecastWindow({ name: 'sevenDay', lengthHours: 240, utilization: 20, resetsAt: new Date(NOW + 8 * 24 * HOUR).toISOString() }, NOW)
    expect(custom?.projectedAtReset).toBeCloseTo(100)
  })

  it('gives no forecast for a window with no length', () => {
    expect(forecastWindow({ name: 'fiveHour', lengthHours: 0, utilization: 50, resetsAt: new Date(NOW + HOUR).toISOString() }, NOW)).toBeUndefined()
  })
})
