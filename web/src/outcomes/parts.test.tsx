import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { createApiClient } from '../api/client'
import { ApiProvider } from '../api/context'
import type { KpiInfo, KpiSeries, Reading, Readings } from '../api/outcomesApi'
import { I18nProvider } from '../i18n'
import { ToastProvider } from '../shared/Toasts'
import { en } from '../i18n/en'
import { KpiCard } from './KpiCard'
import { ReadingsSection } from './ReadingsSection'
import { refusalOf } from './refusal'

const count: KpiInfo = { id: 'k', label: 'Open bugs', kind: 'product', unit: 'count', goodWhen: 'lower' }
const draw = (series: KpiSeries) =>
  render(
    <I18nProvider initial="en">
      <QueryClientProvider client={new QueryClient()}>
        <ApiProvider client={createApiClient(() => Promise.reject(new Error('no node here')))}>
          <ToastProvider>
            <KpiCard org="acme" series={series} until="2026-11-24" days={30} />
          </ToastProvider>
        </ApiProvider>
      </QueryClientProvider>
    </I18nProvider>,
  )

describe('a KPI card', () => {
  it('breaks the line on a day with no number: the days between two numbers are a gap, never a 0', () => {
    draw({ kpi: count, points: [{ date: '2026-11-10', value: 5 }, { date: '2026-11-20', value: 7 }] })
    const path = document.querySelector('.ev-line')?.getAttribute('d') ?? ''
    expect(path.match(/M/g)).toHaveLength(2)
    expect(path, 'no segment joins the two numbers through the days between').not.toContain('L')
  })

  it('lists the numbers instead of drawing them when one is below zero, which a chart from zero cannot show', () => {
    draw({ kpi: { ...count, unit: 'money', currency: 'JPY' }, points: [{ date: '2026-11-10', value: -1200 }, { date: '2026-11-20', value: 300 }] })
    expect(document.querySelector('.ev-line')).toBeNull()
    expect(screen.getByRole('list', { name: 'Numbers' })).toHaveTextContent('-¥1,200')
    expect(screen.getByRole('list', { name: 'Numbers' })).toHaveTextContent('¥300')
  })

  it('calls a change that is written as 0 no movement: unchanged, and not coloured', () => {
    draw({ kpi: { ...count, unit: 'percent', goodWhen: 'higher' }, points: [{ date: '2026-11-20', value: 50.004 }], before: { date: '2026-10-31', value: 50 } })
    expect(screen.getByText('Unchanged since Oct 31 (50%).')).toBeInTheDocument()
    expect(document.querySelector('.oc-move.is-better, .oc-move.is-worse')).toBeNull()
  })

  it('says a KPI with only an older number has none in the period, and gives the older one with its day', () => {
    draw({ kpi: count, points: [], before: { date: '2026-09-30', value: 12 } })
    expect(screen.getByText('No number in this period.')).toBeInTheDocument()
    expect(screen.getByText('Last number: 12, Sep 30.')).toBeInTheDocument()
    expect(document.querySelector('.oc-value')).toBeNull()
  })

  it('does not colour a movement for a KPI whose good direction the node did not say', () => {
    draw({ kpi: { ...count, goodWhen: undefined }, points: [{ date: '2026-11-20', value: 7 }], before: { date: '2026-10-31', value: 5 } })
    expect(document.querySelector('.oc-move')).not.toBeNull()
    expect(document.querySelector('.oc-move.is-better, .oc-move.is-worse')).toBeNull()
    expect(screen.queryByText('better')).not.toBeInTheDocument()
    expect(screen.queryByText('worse')).not.toBeInTheDocument()
  })
})

const ready: Reading = { team: 'app', epic: 'app#1', kpi: 'k', shippedOn: '2026-10-20', state: 'ready', before: { date: '2026-10-15', value: 10 }, after: { date: '2026-11-10', value: 8 }, change: -2, changeRatio: -0.2, overlapping: [] }
const readings = (...list: Reading[]): Readings => ({ org: 'acme', generatedAt: '2026-11-24T12:00:00Z', readings: list })
const read = (value: Readings) =>
  render(
    <I18nProvider initial="en">
      <ReadingsSection readings={value} kpis={[count]} teamLabels={new Map()} />
    </I18nProvider>,
  )

describe('the readings', () => {
  it('names no overlap when there is none, and colours the movement by the KPI’s good direction', () => {
    read(readings(ready))
    expect(screen.queryByText(/Cannot be told apart/)).not.toBeInTheDocument()
    expect(document.querySelector('.oc-move.is-better')).not.toBeNull()
    expect(document.querySelector('.oc-move')).toHaveTextContent('Fell −2 (−20%)better')
  })

  it('calls a reading whose change is written as 0 unchanged, and not coloured', () => {
    read(readings({ ...ready, before: { date: '2026-10-15', value: 10 }, after: { date: '2026-11-10', value: 10.001 }, change: 0.001, changeRatio: 0.0001 }, ))
    expect(document.querySelector('.oc-move')).toHaveTextContent(/^Unchanged 0/)
    expect(document.querySelector('.oc-move.is-better, .oc-move.is-worse')).toBeNull()
    expect(document.querySelector('.oc-move')).not.toHaveTextContent('+')
  })

  it('names the overlapping work by its title where the answer has one', () => {
    read(readings({ ...ready, overlapping: ['app#2'] }, { ...ready, epic: 'app#2', epicTitle: 'Retry banner', overlapping: ['app#1'] }))
    expect(screen.getAllByText(/Cannot be told apart from: Retry banner, which shipped in the same span\./)).toHaveLength(1)
  })

  it('draws no numbers for a reading that is not ready, whatever it carries', () => {
    read(readings({ ...ready, state: 'tooEarly', readableFrom: '2026-12-04', before: undefined, after: undefined, change: undefined, changeRatio: undefined }))
    expect(document.querySelector('.oc-move')).toBeNull()
    expect(screen.getByText('Too early: a change needs 14 days to show. It can be read from Dec 4.')).toBeInTheDocument()
  })

  it('says an org with no aims has nothing to read, not a spinner', () => {
    read(readings())
    expect(screen.getByText('No team of this org has an aim yet, so there is nothing to read.')).toBeInTheDocument()
  })
})

describe('what a refusing reading says when its detail is missing', () => {
  const states = en.outcomes.readings.states
  const date = (day: string): string => `<${day}>`
  const base: Reading = { team: 'app', epic: 'e', kpi: 'k', state: 'unknown', overlapping: [] }

  it('says the state cannot be shown, and guesses nothing, for a state that needs a day or a count it was not given', () => {
    for (const state of ['noBaseline', 'tooEarly', 'tooFew'] as const) {
      expect(refusalOf({ ...base, state }, states, date), state).toBe(states.unknown)
    }
    expect(refusalOf({ ...base, state: 'tooFew', points: 2 }, states, date)).toBe(states.unknown)
  })

  it('says what is missing when it has the detail', () => {
    expect(refusalOf({ ...base, state: 'noBaseline', shippedOn: '2026-11-01' }, states, date)).toBe(states.noBaseline('<2026-11-01>'))
    expect(refusalOf({ ...base, state: 'tooEarly', readableFrom: '2026-12-04' }, states, date)).toBe(states.tooEarly('<2026-12-04>'))
    expect(refusalOf({ ...base, state: 'tooFew', points: 2, needs: 3 }, states, date)).toBe(states.tooFew(2, 3))
    expect(refusalOf({ ...base, state: 'ready' }, states, date)).toBeUndefined()
  })
})
