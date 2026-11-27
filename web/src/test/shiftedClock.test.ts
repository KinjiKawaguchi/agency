import { installShiftedClock } from './shiftedClock'

const holder = () => ({ Date })

describe('a clock that starts at a chosen moment', () => {
  it('is left alone without a start', () => {
    const target = holder()
    installShiftedClock(target, undefined)
    installShiftedClock(target, '')
    expect(target.Date).toBe(Date)
  })

  it('starts at the moment and runs on from it, for `new Date()` and `Date.now()`', async () => {
    const target = holder()
    installShiftedClock(target, '2026-11-28T12:00:00Z')
    const first = target.Date.now()
    expect(Math.abs(first - Date.parse('2026-11-28T12:00:00Z'))).toBeLessThan(2_000)
    expect(Math.abs(new target.Date().getTime() - first)).toBeLessThan(2_000)
    await new Promise((resolve) => setTimeout(resolve, 30))
    expect(target.Date.now()).toBeGreaterThan(first + 20)
  })

  it('does not move a moment that is given to it, a parse, or what it is an instance of', () => {
    const target = holder()
    installShiftedClock(target, '2030-01-01T00:00:00Z')
    expect(new target.Date('2020-02-03T04:05:06Z').toISOString()).toBe('2020-02-03T04:05:06.000Z')
    expect(new target.Date(2020, 1, 3).getFullYear()).toBe(2020)
    expect(target.Date.parse('2021-01-01T00:00:00Z')).toBe(Date.parse('2021-01-01T00:00:00Z'))
    expect(new target.Date() instanceof Date).toBe(true)
    expect(target.Date.UTC(2020, 0, 1)).toBe(Date.UTC(2020, 0, 1))
  })

  it('refuses a start that is not a time, rather than starting at nothing', () => {
    expect(() => installShiftedClock(holder(), 'next tuesday')).toThrow('AGENCY_TEST_NOW is not a time')
  })
})
