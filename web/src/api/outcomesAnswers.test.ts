import { MalformedAnswer } from './evaluationAnswers'
import { readOutcomes, readReadings } from './outcomesAnswers'
import { sampleOutcomes, sampleReadings } from './outcomesSamples'

const clone = (value: unknown): Record<string, unknown> => JSON.parse(JSON.stringify(value)) as Record<string, unknown>

describe('the node’s answer about the numbers', () => {
  it('reads the shared sample as it is: only the days that have a number, a baseline where there is one', () => {
    const outcomes = readOutcomes(sampleOutcomes)
    expect(outcomes.series.map((one) => [one.kpi.id, one.points.length, one.before?.value])).toEqual([['revenue', 1, 1200000], ['activation', 2, undefined], ['lead-time', 0, undefined]])
    expect(outcomes.series[0]?.kpi).toEqual({ id: 'revenue', label: 'Monthly revenue', kind: 'business', unit: 'money', currency: 'JPY', goodWhen: 'higher' })
    expect(outcomes.series[2]?.points, 'an empty KPI is an empty list, never a 0').toEqual([])
  })

  it('keeps a unit, a good direction, or a state it does not know as unknown, and does not fail', () => {
    const raw = clone(sampleOutcomes)
    const series = raw.series as Array<{ kpi: Record<string, unknown> }>
    series[0]!.kpi.unit = 'furlongs'
    series[1]!.kpi.goodWhen = 'sideways'
    const outcomes = readOutcomes(raw)
    expect(outcomes.series[0]?.kpi.unit).toBe('unknown')
    expect(outcomes.series[0]?.kpi.currency).toBeUndefined()
    expect(outcomes.series[1]?.kpi.goodWhen).toBeUndefined()
    const readings = clone(sampleReadings)
    ;(readings.readings as Array<Record<string, unknown>>)[0]!.state = 'someDayMaybe'
    expect(readReadings(readings).readings[0]).toMatchObject({ state: 'unknown', overlapping: [] })
    expect(readReadings(readings).readings[0]?.before, 'no numbers are drawn from a state this client does not know').toBeUndefined()
  })

  it.each([
    ['a value that is not a number', (raw: Record<string, unknown>) => ((raw.series as Array<{ points: Array<Record<string, unknown>> }>)[1]!.points[0]!.value = '41.5')],
    ['a value that is infinite', (raw: Record<string, unknown>) => ((raw.series as Array<{ points: Array<Record<string, unknown>> }>)[1]!.points[0]!.value = 1e999)],
    ['a day that is not a day', (raw: Record<string, unknown>) => ((raw.series as Array<{ points: Array<Record<string, unknown>> }>)[1]!.points[0]!.date = 'yesterday')],
    ['a day with the shape of one that the calendar does not have (month 13)', (raw: Record<string, unknown>) => ((raw.series as Array<{ points: Array<Record<string, unknown>> }>)[1]!.points[0]!.date = '2026-13-45')],
    ['a day that does not exist (February 30)', (raw: Record<string, unknown>) => ((raw.series as Array<{ points: Array<Record<string, unknown>> }>)[1]!.points[0]!.date = '2026-02-30')],
    ['a period that ends on no day', (raw: Record<string, unknown>) => (raw.until = '2026-00-10')],
    ['an answer that is not an object', () => undefined],
    ['points that are not a list', (raw: Record<string, unknown>) => ((raw.series as Array<Record<string, unknown>>)[1]!.points = 'none')],
    ['a series that is missing its KPI', (raw: Record<string, unknown>) => delete (raw.series as Array<Record<string, unknown>>)[0]!.kpi],
  ])('refuses the whole answer for %s, and never fills in', (_name, spoil) => {
    const raw = clone(sampleOutcomes)
    spoil(raw)
    expect(() => readOutcomes(_name === 'an answer that is not an object' ? 42 : raw)).toThrow(MalformedAnswer)
  })

  it('reads every state of the shared readings, with numbers only where the state is ready', () => {
    const { readings } = readReadings(sampleReadings)
    expect(readings.map((one) => one.state)).toEqual(['ready', 'ready', 'notShipped', 'shipDateUnknown', 'noBaseline', 'noFollowUp', 'tooEarly', 'tooFew'])
    expect(readings[0]).toMatchObject({ change: 4, changeRatio: 0.1, overlapping: ['app#12'], before: { value: 40 }, after: { value: 44 } })
    for (const reading of readings.filter((one) => one.state !== 'ready')) {
      expect([reading.before, reading.after, reading.change, reading.changeRatio], reading.state).toEqual([undefined, undefined, undefined, undefined])
      expect(reading.overlapping).toEqual([])
    }
    expect(readings[6]?.readableFrom).toBe('2026-12-04')
    expect([readings[7]?.points, readings[7]?.needs]).toEqual([2, 3])
  })

  it('drops numbers that came with a reading that is not ready', () => {
    const raw = clone(sampleReadings)
    const list = raw.readings as Array<Record<string, unknown>>
    list[2] = { ...list[2], change: 9, before: { date: '2026-10-01', value: 1 }, after: { date: '2026-11-01', value: 10 } }
    expect(readReadings(raw).readings[2]).not.toHaveProperty('change')
    expect(readReadings(raw).readings[2]).not.toHaveProperty('before')
  })

  it('refuses a reading whose ship day is not a day of the calendar', () => {
    const raw = clone(sampleReadings)
    ;(raw.readings as Array<Record<string, unknown>>)[0]!.shippedOn = '2026-13-01'
    expect(() => readReadings(raw)).toThrow(MalformedAnswer)
    expect(() => readReadings({ ...clone(sampleReadings), readings: 'none' })).toThrow(MalformedAnswer)
  })

  it('refuses a ready reading whose numbers are not numbers', () => {
    const raw = clone(sampleReadings)
    ;((raw.readings as Array<Record<string, unknown>>)[0]!.before as Record<string, unknown>).value = null
    expect(() => readReadings(raw)).toThrow(MalformedAnswer)
  })
})
