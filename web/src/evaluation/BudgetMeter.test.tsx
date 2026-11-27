import { render, screen } from '@testing-library/react'
import sample from '../../../testdata/budget.sample.json' with { type: 'json' }
import type { BudgetPart, BudgetStatus } from '../api/clientApi'
import { readBudget } from '../api/budgetAnswers'
import { I18nProvider } from '../i18n'
import { BudgetMeter } from './BudgetMeter'

const ok: BudgetPart = { capUsd: 100, spentUsd: 40, state: 'ok' }

const draw = (team: BudgetPart, org: BudgetPart = ok, locale: 'en' | 'ja' = 'en') => {
  const status: BudgetStatus = { weekStart: '2026-11-23', team, org }
  return render(
    <I18nProvider initial={locale}>
      <BudgetMeter status={status} />
    </I18nProvider>,
  )
}

const fill = (container: HTMLElement): HTMLElement => container.querySelector('.ev-meter span') as HTMLElement

describe('the weekly budget, as the node computed it', () => {
  it('says which week it is and what was spent of the cap, within the budget', () => {
    const { container } = draw({ capUsd: 200, spentUsd: 40, state: 'ok' })
    expect(screen.getByText('$40 of $200 this week, from Monday 2026-11-23')).toBeInTheDocument()
    expect(screen.getByText('Within the budget')).toBeInTheDocument()
    expect(fill(container).className).toBe('')
    expect(fill(container).style.width).toBe('20%')
  })

  it.each([
    ['warning', 'Close to the budget'],
    ['over', 'Over the budget'],
  ] as const)('says %s in the node’s word and colors the fill by it', (state, words) => {
    const { container } = draw({ capUsd: 100, spentUsd: state === 'over' ? 120 : 86.4, state })
    expect(screen.getByText(words)).toBeInTheDocument()
    expect(fill(container).className).toBe(state)
    expect(screen.getByRole('img')).toHaveAttribute('aria-label', expect.stringContaining(words))
  })

  it('stops the fill at the whole width when more is spent than the cap', () => {
    const { container } = draw({ capUsd: 100, spentUsd: 250, state: 'over' })
    expect(fill(container).style.width).toBe('100%')
  })

  it('does not decide the state: a team that is over because its org is over is over, though it spent less than its cap, and the org is said', () => {
    const { container } = draw({ capUsd: 300, spentUsd: 147.8, state: 'over' }, { capUsd: 500, spentUsd: 640, state: 'over' })
    expect(screen.getByText('Over the budget')).toBeInTheDocument()
    expect(fill(container).className).toBe('over')
    expect(screen.getByText('The org is over its weekly budget: $640 of $500.')).toBeInTheDocument()
  })

  it('does not decide the state the other way either: what the node calls ok is ok, whatever was spent', () => {
    const { container } = draw({ capUsd: 100, spentUsd: 120, state: 'ok' })
    expect(screen.getByText('Within the budget')).toBeInTheDocument()
    expect(screen.queryByText('Over the budget')).not.toBeInTheDocument()
    expect(fill(container).className).toBe('')
  })

  it('says nothing of the org while the org is within its budget', () => {
    draw(ok, { capUsd: 500, spentUsd: 100, state: 'ok' })
    expect(screen.queryByText(/The org is/)).not.toBeInTheDocument()
  })

  it('says the org when it is close to its budget', () => {
    draw(ok, { capUsd: 500, spentUsd: 411, state: 'warning' })
    expect(screen.getByText('The org is close to its weekly budget: $411 of $500.')).toBeInTheDocument()
  })

  it('says what a team spent is not known when its usage could not be read: no figure, no bar, no state, never the node’s 0', () => {
    const { container } = draw({ capUsd: 100, spentUsd: 0, state: 'ok', partial: true })
    expect(screen.getByText('Not known this week, from Monday 2026-11-23')).toBeInTheDocument()
    expect(screen.getByText('This team’s usage could not be read, so what it spent this week is not known.')).toBeInTheDocument()
    expect(container.querySelector('.ev-meter')).toBeNull()
    expect(screen.queryByText(/\$0/)).not.toBeInTheDocument()
    expect(screen.queryByText('Within the budget')).not.toBeInTheDocument()
  })

  it('says the org’s amount is at least that much when part of it was not read, also when the org is within its budget', () => {
    draw(ok, { capUsd: 500, spentUsd: 311, state: 'ok', partial: true })
    expect(screen.getByText('Some of the org’s usage could not be read: at least $311 of $500 this week.')).toBeInTheDocument()
    expect(screen.getByText('Within the budget')).toBeInTheDocument()
  })

  it('says the org is over its budget with the amount as a lower bound when part of it was not read', () => {
    draw(ok, { capUsd: 500, spentUsd: 640, state: 'over', partial: true })
    expect(screen.getByText('The org is over its weekly budget: $640 of $500. Some usage could not be read, so it is at least this much.')).toBeInTheDocument()
  })

  it('draws the shared sample: the team close to its cap, and the org whose usage was only partly read', () => {
    const { container } = render(
      <I18nProvider initial="en">
        <BudgetMeter status={readBudget(sample)} />
      </I18nProvider>,
    )
    expect(screen.getByText('$163 of $200 this week, from Monday 2026-11-23')).toBeInTheDocument()
    expect(screen.getByText('Close to the budget')).toBeInTheDocument()
    expect(screen.getByText('Some of the org’s usage could not be read: at least $311 of $500 this week.')).toBeInTheDocument()
    expect(fill(container).style.width).toBe('81.6%')
  })

  it('says a team’s unknown spend and the org’s lower bound in Japanese', () => {
    draw({ capUsd: 100, spentUsd: 0, state: 'ok', partial: true }, { capUsd: 500, spentUsd: 311, state: 'ok', partial: true }, 'ja')
    expect(screen.getByText('このチームの使用量を読めなかったため、今週の使用額は分かりません。')).toBeInTheDocument()
    expect(screen.getByText('組織の使用量に読めなかったものがあります: 今週は $500 のうち、少なくとも $311。')).toBeInTheDocument()
  })

  it('draws no meter for a team with no cap, and nothing at all while its org is within its budget', () => {
    const { container } = draw({ spentUsd: 40, state: 'ok' })
    expect(container).toBeEmptyDOMElement()
  })

  it.each([
    ['over', 'The org is over its weekly budget: $640 of $500.'],
    ['warning', 'The org is close to its weekly budget: $411 of $500.'],
  ] as const)('says the org is %s for a team with no cap, which is over with it, and draws no meter', (state, words) => {
    const { container } = draw({ spentUsd: 40, state: state === 'over' ? 'over' : 'ok' }, { capUsd: 500, spentUsd: state === 'over' ? 640 : 411, state })
    expect(screen.getByText(words)).toBeInTheDocument()
    expect(container.querySelector('.ev-meter')).toBeNull()
  })

  it('says the org’s lower bound for a team with no cap when part of the org’s usage was not read', () => {
    draw({ spentUsd: 0, state: 'ok' }, { capUsd: 500, spentUsd: 311, state: 'ok', partial: true })
    expect(screen.getByText('Some of the org’s usage could not be read: at least $311 of $500 this week.')).toBeInTheDocument()
  })

  it('does not fill the bar of a cap of nothing unless the node says over', () => {
    const { container } = draw({ capUsd: 0, spentUsd: 0, state: 'ok' })
    expect(fill(container).style.width).toBe('0%')
  })

  it('says it in Japanese', () => {
    draw({ capUsd: 100, spentUsd: 120, state: 'over' }, { capUsd: 100, spentUsd: 120, state: 'over' }, 'ja')
    expect(screen.getByText('予算を超えています')).toBeInTheDocument()
    expect(screen.getByText('組織は週の予算を超えています: $100 のうち $120。')).toBeInTheDocument()
    expect(screen.getByRole('img')).toHaveAttribute('aria-label', expect.stringContaining('週の予算'))
  })
})
