import { render, screen } from '@testing-library/react'
import type { Funding } from '../api/clientApi'
import { I18nProvider } from '../i18n'
import { FundingCard } from './FundingCard'

const NOW = Date.parse('2026-11-25T12:00:00Z')

const source = (change: Partial<Funding>): Funding => ({ id: 'plan', label: 'Plan', kind: 'subscription', windows: [], ...change })

const show = (funding: Funding) =>
  render(
    <I18nProvider initial="en">
      <FundingCard source={funding} now={NOW} />
    </I18nProvider>,
  )

describe('FundingCard', () => {
  it('says the usage could not be read when the node reports no windows', () => {
    show(source({ windows: [] }))
    expect(screen.getByText('The usage of this source could not be read.')).toBeInTheDocument()
    expect(screen.queryByText(/has not started/)).not.toBeInTheDocument()
  })

  it('says a window has not started when only the other one is reported', () => {
    show(source({ windows: [{ name: 'sevenDay', lengthHours: 168, utilization: 40, resetsAt: new Date(NOW + 84 * 3_600_000).toISOString() }] }))
    expect(screen.getByText('Five-hour window has not started.')).toBeInTheDocument()
  })

  it('projects a window from the length the node reports', () => {
    // Half of a 10-day window is gone at 60%, so about 120% by the reset: it will run out.
    show(source({ windows: [{ name: 'sevenDay', lengthHours: 240, utilization: 60, resetsAt: new Date(NOW + 120 * 3_600_000).toISOString() }] }))
    expect(screen.getByText('Will run out')).toBeInTheDocument()
    expect(screen.getByRole('img', { name: 'Weekly window: 60% used, about 120% at the reset' })).toBeInTheDocument()
  })
})
