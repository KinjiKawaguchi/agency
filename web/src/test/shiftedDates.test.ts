import { nextClockChange, shiftedRuns } from './shiftedDates'

const TODAY = new Date('2026-11-27T10:00:00Z')
const runs = shiftedRuns(TODAY)
const nowOf = (name: string): string | undefined => runs.find((run) => run.name === name)?.now

describe('the moments the weekly run starts its clock at', () => {
  it('has each of the next seven days, so that every weekday comes', () => {
    const days = runs.filter((run) => /^[A-Z][a-z]+day 20/.test(run.name))
    expect(days.map((run) => run.name.split(' ')[0])).toEqual(['Saturday', 'Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday'])
    expect(days.map((run) => run.now.slice(0, 10))).toEqual(['2026-11-28', '2026-11-29', '2026-11-30', '2026-12-01', '2026-12-02', '2026-12-03', '2026-12-04'])
    expect(new Set(days.map((run) => run.timeZone))).toEqual(new Set(['UTC']))
  })

  it('has the end of the month and the start of the next, and the end of the year and the start of the next', () => {
    expect(nowOf('end of the month')).toBe('2026-11-30T23:30:00.000Z')
    expect(nowOf('start of the next month')).toBe('2026-12-01T00:30:00.000Z')
    expect(nowOf('end of the year')).toBe('2026-12-31T23:30:00.000Z')
    expect(nowOf('start of the next year')).toBe('2027-01-01T00:30:00.000Z')
  })

  it('takes the end that is still to come: on the last day, after its end, the next month’s; after 31 December, next year’s', () => {
    const late = shiftedRuns(new Date('2026-11-30T23:45:00Z'))
    expect(late.find((run) => run.name === 'end of the month')?.now).toBe('2026-12-31T23:30:00.000Z')
    const newYear = shiftedRuns(new Date('2027-01-02T00:00:00Z'))
    expect(newYear.find((run) => run.name === 'end of the year')?.now).toBe('2027-12-31T23:30:00.000Z')
  })

  it('has, for each zone with a clock change, ten minutes before the next one, in that zone', () => {
    expect(runs.filter((run) => run.name.startsWith('clock change'))).toEqual([
      { name: 'clock change in America/New_York', now: '2027-03-14T06:50:00.000Z', timeZone: 'America/New_York' },
      { name: 'clock change in Europe/London', now: '2027-03-28T00:50:00.000Z', timeZone: 'Europe/London' },
      { name: 'clock change in Australia/Sydney', now: '2027-04-03T15:50:00.000Z', timeZone: 'Australia/Sydney' },
    ])
  })

  it('has no clock change for a zone that does not change its clock', () => {
    expect(nextClockChange('Asia/Tokyo', TODAY.getTime())).toBeUndefined()
    expect(shiftedRuns(TODAY, ['Asia/Tokyo', 'UTC']).filter((run) => run.name.startsWith('clock change'))).toEqual([])
  })

  it('finds the change after a moment, and not the one the moment is in', () => {
    const march = Date.parse('2027-03-14T06:50:00Z')
    expect(new Date(nextClockChange('America/New_York', march) ?? 0).toISOString()).toBe('2027-03-14T07:00:00.000Z')
    expect(new Date(nextClockChange('America/New_York', Date.parse('2027-03-14T07:00:00Z')) ?? 0).toISOString()).toBe('2027-11-07T06:00:00.000Z')
  })
})
