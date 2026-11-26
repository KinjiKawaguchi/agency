import { durationAxis, formatDuration } from './duration'

const units = { minute: 'm', hour: 'h', day: 'd' }

describe('formatDuration', () => {
  it('uses minutes under an hour, hours under two days, and days above', () => {
    expect(formatDuration(0, units)).toBe('0m')
    expect(formatDuration(45 * 60, units)).toBe('45m')
    expect(formatDuration(5400, units)).toBe('1.5h')
    expect(formatDuration(12 * 3600, units)).toBe('12h')
    expect(formatDuration(36 * 3600, units)).toBe('36h')
    expect(formatDuration(3 * 86_400, units)).toBe('3d')
    expect(formatDuration(5 * 86_400 + 43_200, units)).toBe('5.5d')
  })
})

describe('durationAxis', () => {
  it('tops out at a round number of one unit and writes every tick in it', () => {
    const hours = durationAxis(5.5 * 3600, units)
    expect(hours.max).toBe(10 * 3600)
    expect([hours.max, hours.max / 2, 0].map(hours.format)).toEqual(['10h', '5h', '0h'])
    const days = durationAxis(3 * 86_400, units)
    expect(days.max).toBe(5 * 86_400)
    expect(days.format(days.max / 2)).toBe('2.5d')
    const minutes = durationAxis(20 * 60, units)
    expect(minutes.max).toBe(20 * 60)
    expect(minutes.format(0)).toBe('0m')
  })

  it('still has an axis when there is nothing to draw', () => {
    expect(durationAxis(0, units).max).toBeGreaterThan(0)
  })
})
