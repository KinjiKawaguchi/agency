import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useDays } from './useDays'

function Days({ count }: { count: number }) {
  const days = useDays(count)
  return (
    <div role="group" aria-label="days" onKeyDown={days.onKeyDown}>
      {Array.from({ length: count }, (_, index) => (
        <div key={index} role="button" aria-label={`day ${index + 1}`} {...days.itemProps(index)} />
      ))}
    </div>
  )
}

const tabbable = (): string[] => screen.getAllByRole('button').filter((one) => one.tabIndex === 0).map((one) => one.getAttribute('aria-label') ?? '')

describe('the days of a chart', () => {
  it('are one tab stop, on the newest day', () => {
    render(<Days count={5} />)
    expect(tabbable()).toEqual(['day 5'])
  })

  it('are walked with the arrow keys, Home, and End, and stop at the ends', async () => {
    const user = userEvent.setup()
    render(<Days count={5} />)
    await user.tab()
    expect(screen.getByRole('button', { name: 'day 5' })).toHaveFocus()
    await user.keyboard('{ArrowLeft}{ArrowLeft}')
    expect(screen.getByRole('button', { name: 'day 3' })).toHaveFocus()
    expect(tabbable()).toEqual(['day 3'])
    await user.keyboard('{Home}')
    expect(screen.getByRole('button', { name: 'day 1' })).toHaveFocus()
    await user.keyboard('{ArrowLeft}')
    expect(screen.getByRole('button', { name: 'day 1' })).toHaveFocus()
    await user.keyboard('{End}{ArrowRight}')
    expect(screen.getByRole('button', { name: 'day 5' })).toHaveFocus()
  })

  it('come back to the day they were left on when tabbed to again, and keep a day when the click chose it', async () => {
    const user = userEvent.setup()
    render(<><button>before</button><Days count={5} /><button>after</button></>)
    await user.click(screen.getByRole('button', { name: 'day 2' }))
    await user.click(screen.getByRole('button', { name: 'before' }))
    await user.tab()
    expect(screen.getByRole('button', { name: 'day 2' })).toHaveFocus()
    await user.tab()
    expect(screen.getByRole('button', { name: 'after' })).toHaveFocus()
  })

  it('stay inside the chart when the period gets shorter', () => {
    const { rerender } = render(<Days count={30} />)
    expect(tabbable()).toEqual(['day 30'])
    rerender(<Days count={14} />)
    expect(tabbable()).toEqual(['day 14'])
  })
})
