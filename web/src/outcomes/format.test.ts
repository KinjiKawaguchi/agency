import type { KpiInfo } from '../api/outcomesApi'
import { formatKpiAxis, formatKpiChange, formatKpiValue } from './format'
import { movementOf } from './movement'

const UNITS = { minute: 'm', hour: 'h', day: 'd' }
const kpi = (unit: KpiInfo['unit'], extra: Partial<KpiInfo> = {}): KpiInfo => ({ id: 'k', label: 'K', kind: 'business', unit, ...extra })

describe('a KPI’s number in its own unit', () => {
  it('writes money in its own currency with that currency’s decimals, and never converts it', () => {
    expect(formatKpiValue(1350000, kpi('money', { currency: 'JPY' }), 'en', UNITS)).toBe('¥1,350,000')
    expect(formatKpiValue(1350000, kpi('money', { currency: 'JPY' }), 'ja', UNITS)).toBe('￥1,350,000')
    expect(formatKpiValue(1234.5, kpi('money', { currency: 'USD' }), 'en', UNITS)).toBe('$1,234.50')
    expect(formatKpiValue(1234.5, kpi('money', { currency: 'EUR' }), 'en', UNITS)).toBe('€1,234.50')
  })

  it('shows a currency code it cannot format as a plain number with the code beside it', () => {
    expect(formatKpiValue(5, kpi('money', { currency: 'NOT-A-CODE' }), 'en', UNITS)).toBe('5 NOT-A-CODE')
    expect(formatKpiValue(5, kpi('money'), 'en', UNITS)).toBe('5')
  })

  it('writes a percent as a number of percent, never clipped', () => {
    for (const [value, text] of [[41.5, '41.5%'], [120, '120%'], [-3, '-3%'], [0, '0%'], [33.333, '33.33%']] as const) {
      expect(formatKpiValue(value, kpi('percent'), 'en', UNITS)).toBe(text)
    }
  })

  it('writes a ratio as a multiple, a count as a plain number, and seconds as a duration', () => {
    expect(formatKpiValue(1.5, kpi('ratio'), 'en', UNITS)).toBe('1.5×')
    expect(formatKpiValue(0.8, kpi('ratio'), 'en', UNITS)).toBe('0.8×')
    expect(formatKpiValue(12345, kpi('count'), 'en', UNITS)).toBe('12,345')
    expect(formatKpiValue(3 * 86_400 + 4 * 3_600, kpi('seconds'), 'en', UNITS)).toBe('3.2d')
    expect(formatKpiValue(90 * 60, kpi('seconds'), 'en', UNITS)).toBe('1.5h')
    expect(formatKpiValue(-7200, kpi('seconds'), 'en', UNITS)).toBe('−2h')
  })

  it('shows a unit it does not know as a plain number', () => {
    expect(formatKpiValue(1234.5, kpi('unknown'), 'en', UNITS)).toBe('1,234.5')
  })

  it('writes a change with its sign, in the unit', () => {
    expect(formatKpiChange(4, kpi('percent'), 'en', UNITS)).toBe('+4 points')
    expect(formatKpiChange(-2.5, kpi('percent'), 'ja', UNITS)).toBe('−2.5ポイント')
    expect(formatKpiChange(-3, kpi('count'), 'en', UNITS)).toBe('−3')
    expect(formatKpiChange(0, kpi('count'), 'en', UNITS)).toBe('0')
    expect(formatKpiChange(0.004, kpi('percent'), 'en', UNITS), 'a change that is written as 0 has no sign').toBe('0 points')
    expect(formatKpiChange(-0.004, kpi('percent'), 'ja', UNITS)).toBe('0ポイント')
    expect(formatKpiChange(0.01, kpi('percent'), 'en', UNITS)).toBe('+0.01 points')
    expect(formatKpiChange(0.4, kpi('money', { currency: 'JPY' }), 'en', UNITS)).toBe('¥0')
    expect(formatKpiChange(150000, kpi('money', { currency: 'JPY' }), 'en', UNITS)).toBe('+¥150,000')
  })
})

describe('how a number moved', () => {
  it('tells the direction, and from the KPI’s good direction whether that is better or worse', () => {
    expect(movementOf(40, 44, 'higher')).toEqual({ direction: 'rose', quality: 'better' })
    expect(movementOf(40, 44, 'lower')).toEqual({ direction: 'rose', quality: 'worse' })
    expect(movementOf(44, 40, 'higher')).toEqual({ direction: 'fell', quality: 'worse' })
    expect(movementOf(44, 40, 'lower')).toEqual({ direction: 'fell', quality: 'better' })
  })

  it('says nothing of better or worse for no change, or when the node did not say which way is good', () => {
    expect(movementOf(40, 40, 'higher')).toEqual({ direction: 'unchanged', quality: undefined })
    expect(movementOf(40, 44, undefined)).toEqual({ direction: 'rose', quality: undefined })
  })
})

describe('the ticks of an axis', () => {
  it('are short where room is short, and the same as the values elsewhere', () => {
    expect(formatKpiAxis(2000000, kpi('money', { currency: 'JPY' }), 'en', UNITS)).toBe('¥2M')
    expect(formatKpiAxis(1200, kpi('count'), 'en', UNITS)).toBe('1.2K')
    expect(formatKpiAxis(50, kpi('percent'), 'en', UNITS)).toBe('50%')
    expect(formatKpiAxis(5, kpi('money', { currency: 'NOT-A-CODE' }), 'en', UNITS)).toBe('5 NOT-A-CODE')
  })
})
