import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

const funding = async (name: string) => within((await screen.findByRole('heading', { level: 3, name })).closest('article') as HTMLElement)

describe('costs', () => {
  it('projects each window of a subscription to its reset from the pace so far', async () => {
    renderApp({ path: '/costs' })
    const personal = await funding('Personal plan')
    expect(personal.getByRole('img', { name: 'Weekly window: 52% used, about 91% at the reset' })).toBeInTheDocument()
    expect(personal.getByText('On pace')).toBeInTheDocument()
    expect(personal.getByText('Will run out')).toBeInTheDocument()
    expect(personal.getByText(/The limit is reached around/)).toBeInTheDocument()
    expect(personal.getByText(/^Read /)).toBeInTheDocument()
  })

  it('says a window has not started and warns when one will go unused', async () => {
    renderApp({ path: '/costs' })
    const lab = await funding('Lab plan')
    expect(lab.getByText('Five-hour window has not started.')).toBeInTheDocument()
    expect(lab.getByText('Will go unused')).toBeInTheDocument()
  })

  it('shows token use by day and by person for each team', async () => {
    renderApp({ path: '/costs' })
    expect(await screen.findByRole('heading', { level: 2, name: 'App team tokens' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 2, name: 'Site team tokens' })).toBeInTheDocument()
    const tables = await screen.findAllByRole('table')
    const app = within(tables[0] as HTMLElement)
    expect(app.getAllByRole('rowheader').map((cell) => cell.textContent).toSorted()).toEqual(['Backend', 'Docs', 'Frontend', 'Lead'])
    expect(screen.getAllByText(/Priced as if billed by the API/).length).toBeGreaterThan(0)
  })

  it('names every person in the legend and in each day’s tooltip', async () => {
    renderApp({ path: '/costs' })
    const legend = (await screen.findAllByRole('list'))[0] as HTMLElement
    expect(within(legend).getAllByRole('listitem').map((item) => item.textContent)).toEqual(['Lead', 'Backend', 'Frontend', 'Docs'])
  })

  it('redraws the chart for a longer period', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/costs' })
    const columns = async () => ((await screen.findAllByRole('group', { name: 'Cost by day' }))[0] as HTMLElement).querySelectorAll('.column').length
    expect(await columns()).toBe(14)
    await user.click(screen.getAllByRole('button', { name: '30 days' })[0] as HTMLElement)
    await waitFor(async () => expect(await columns()).toBe(30))
    expect(screen.getAllByRole('button', { name: '30 days' })[0]).toHaveAttribute('aria-pressed', 'true')
  })
})
