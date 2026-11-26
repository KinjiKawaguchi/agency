import type { NumberEntry } from '../api/outcomesApi'
import { checkNumber, replacedIn } from './numberInput'

const TODAY = '2026-11-24'
const form = (over: Partial<{ date: string; value: string; note: string }> = {}) => ({ date: '2026-11-20', value: '1350000', note: '', ...over })

describe('what the owner types, checked as the node checks it', () => {
  it('takes a number for a day, with or without a note, trimmed', () => {
    expect(checkNumber(form(), 'revenue', TODAY)).toEqual({ ok: true, input: { kpi: 'revenue', date: '2026-11-20', value: 1350000 } })
    expect(checkNumber(form({ value: ' -3.5 ', note: '  before the invoices  ' }), 'revenue', TODAY)).toEqual({ ok: true, input: { kpi: 'revenue', date: '2026-11-20', value: -3.5, note: 'before the invoices' } })
    expect(checkNumber(form({ value: '0' }), 'k', TODAY)).toMatchObject({ ok: true, input: { value: 0 } })
  })

  it.each([
    ['not a day', { date: '2026-02-30' }],
    ['not a date at all', { date: 'yesterday' }],
    ['empty', { date: '' }],
    ['two days ahead of the node’s today', { date: '2026-11-26' }],
    ['more than ten years ago', { date: '2016-11-23' }],
  ])('refuses a date that is %s', (_name, over) => {
    expect(checkNumber(form(over), 'k', TODAY)).toEqual({ ok: false, problem: 'date' })
  })

  it('takes a date one day ahead, and one exactly ten years back', () => {
    expect(checkNumber(form({ date: '2026-11-25' }), 'k', TODAY).ok).toBe(true)
    expect(checkNumber(form({ date: '2016-11-24' }), 'k', TODAY).ok).toBe(true)
  })

  it.each(['', '  ', 'abc', 'NaN', 'Infinity', '1e999', '1,5', '1e15000', '1000000000000001'])('refuses the value “%s”', (value) => {
    expect(checkNumber(form({ value }), 'k', TODAY)).toEqual({ ok: false, problem: 'value' })
  })

  it('takes the largest value the node takes', () => {
    expect(checkNumber(form({ value: '1000000000000000' }), 'k', TODAY).ok).toBe(true)
  })

  it('refuses a note of more than 200 characters or with a control character, and counts characters, not bytes', () => {
    expect(checkNumber(form({ note: 'a'.repeat(201) }), 'k', TODAY)).toEqual({ ok: false, problem: 'note' })
    expect(checkNumber(form({ note: 'a\u0007b' }), 'k', TODAY)).toEqual({ ok: false, problem: 'note' })
    expect(checkNumber(form({ note: 'a\nb' }), 'k', TODAY)).toEqual({ ok: false, problem: 'note' })
    expect(checkNumber(form({ note: '😀'.repeat(200) }), 'k', TODAY).ok).toBe(true)
    expect(checkNumber(form({ note: 'a'.repeat(200) }), 'k', TODAY).ok).toBe(true)
  })
})

const entry = (date: string, value: number, enteredAt: string, kpi = 'revenue'): NumberEntry => ({ kpi, date, value, source: { kind: 'manual' }, enteredAt, enteredBy: 'owner' })

describe('which entries of a history a later one has replaced', () => {
  it('marks the earlier entry of a day that was entered twice, and keeps the later one', () => {
    const history = [entry('2026-11-20', 2, '2026-11-22T09:00:00Z'), entry('2026-11-19', 1, '2026-11-20T09:00:00Z'), entry('2026-11-20', 1, '2026-11-21T09:00:00Z')]
    expect(replacedIn(history)).toEqual([false, false, true])
  })

  it('does not mix up KPIs, and marks nothing when each day was entered once', () => {
    expect(replacedIn([entry('2026-11-20', 1, '2026-11-21T09:00:00Z'), entry('2026-11-20', 2, '2026-11-20T09:00:00Z', 'other')])).toEqual([false, false])
    expect(replacedIn([])).toEqual([])
  })

  it('marks every earlier one when a day was entered three times', () => {
    const three = [entry('d', 3, '2026-11-23T00:00:00Z'), entry('d', 2, '2026-11-22T00:00:00Z'), entry('d', 1, '2026-11-21T00:00:00Z')].map((one) => ({ ...one, date: '2026-11-20' }))
    expect(replacedIn(three)).toEqual([false, true, true])
  })
})
