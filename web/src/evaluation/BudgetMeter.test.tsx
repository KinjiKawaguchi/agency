import { render, screen } from '@testing-library/react'
import { I18nProvider } from '../i18n'
import { BudgetMeter } from './BudgetMeter'

const draw = (spent: number, budget: number, locale: 'en' | 'ja' = 'en') =>
  render(
    <I18nProvider initial={locale}>
      <BudgetMeter spent={spent} budget={budget} />
    </I18nProvider>,
  )

describe('the weekly budget against the last seven days', () => {
  it('says by how much it is over, and fills the meter in the seal color', () => {
    const { container } = draw(120, 100)
    expect(screen.getByText(/\$20 over$/)).toBeInTheDocument()
    expect(container.querySelector('.ev-meter .over')).not.toBeNull()
  })

  it('says how much is left, and does not call it over', () => {
    const { container } = draw(99.3, 100)
    expect(screen.getByText(/\$0\.70 left$/)).toBeInTheDocument()
    expect(screen.queryByText(/over$/)).not.toBeInTheDocument()
    expect(container.querySelector('.ev-meter .over')).toBeNull()
  })

  it('is not over when it is spent exactly, and says nothing is left', () => {
    draw(100, 100)
    expect(screen.queryByText(/over$/)).not.toBeInTheDocument()
    expect(screen.getByText(/\$0 left$/)).toBeInTheDocument()
  })

  it('says it in Japanese: the amount over, and the meter’s label, in the locale’s words', () => {
    draw(120, 100, 'ja')
    expect(screen.getByText(/\$20 超過$/)).toBeInTheDocument()
    expect(screen.getByRole('img')).toHaveAttribute('aria-label', expect.stringContaining('週の予算'))
    expect(screen.queryByText(/over$/)).not.toBeInTheDocument()
  })

  it('says what is left in Japanese too', () => {
    draw(99.3, 100, 'ja')
    expect(screen.getByText(/残り \$0\.70$/)).toBeInTheDocument()
  })
})
