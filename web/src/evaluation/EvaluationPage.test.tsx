import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

const section = async (name: string) => within((await screen.findByRole('heading', { level: 2, name })).closest('section') as HTMLElement)

describe('evaluation', () => {
  it('shows each team’s headline numbers, new features first', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await section('App team')
    const labels = Array.from(document.querySelectorAll('.ev-team')[0]?.querySelectorAll('.ev-tile-label') ?? []).map((label) => label.textContent)
    expect(labels).toEqual(['New features merged', 'Pull requests merged', 'Lead time, pull request', 'Lead time, issue', 'Main was red', 'Reverts', 'Cost per merged pull request'])
    expect(app.getByText('New features merged').parentElement?.querySelector('.ev-tile-value')?.textContent).toMatch(/^\d+$/)
    expect(screen.getByRole('heading', { level: 2, name: 'Site team' })).toBeInTheDocument()
  })

  it('draws merged pull requests per day, one column for each day, and names every type', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await section('App team')
    expect(app.getByRole('group', { name: 'Merged pull requests per day' }).querySelectorAll('.ev-col')).toHaveLength(14)
    const legend = app.getAllByRole('list')[0] as HTMLElement
    expect(within(legend).getAllByRole('listitem').map((item) => item.textContent)).toEqual(['New feature', 'Fix', 'Refactor', 'Docs', 'Test', 'Chore', 'Other'])
  })

  it('draws the lead time and the cost per merged pull request as lines over the same days', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await section('App team')
    const lead = app.getByRole('group', { name: 'Lead time' })
    expect(lead.querySelectorAll('path.ev-line')).toHaveLength(2)
    expect(lead.querySelectorAll('.ev-cell')).toHaveLength(14)
    expect(app.getByRole('group', { name: 'Cost per merged pull request' }).querySelectorAll('path.ev-line')).toHaveLength(1)
  })

  it('redraws every chart for a longer period and keeps it in the address', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/evaluation', evaluation: true })
    await section('App team')
    await user.click(screen.getByRole('button', { name: '30 days' }))
    await waitFor(() => expect(screen.getAllByRole('group', { name: 'Reverts per day' })[0]?.querySelectorAll('.ev-col')).toHaveLength(30))
    expect(screen.getByRole('button', { name: '30 days' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('gives the same days in a table for anyone who cannot read the colors', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await section('App team')
    const table = within(app.getByRole('table', { hidden: true }))
    expect(table.getAllByRole('rowheader', { hidden: true })).toHaveLength(14)
  })

  it('names the route an agent reads, with the period', async () => {
    renderApp({ path: '/evaluation?days=30', evaluation: true })
    expect(await screen.findByText('The same numbers, for an agent to cite: GET /api/orgs/acme/evaluation?days=30')).toBeInTheDocument()
  })

  it('says so when the node does not report evaluation numbers', async () => {
    renderApp({ path: '/evaluation' })
    expect(await screen.findByText('This node does not report evaluation numbers yet.')).toBeInTheDocument()
  })

  it('says so, in the place of the numbers, when the node could not read them', async () => {
    renderApp({ path: '/evaluation', evaluation: true, evaluationFailsWith: 503 })
    expect(await screen.findByRole('alert')).toHaveTextContent('A team could not report its numbers right now: The evaluation could not be read.')
    expect(screen.getByRole('heading', { level: 1, name: 'Evaluation' })).toBeInTheDocument()
  })

  it('lets the viewer pick an org when they belong to several', async () => {
    renderApp({ path: '/evaluation', evaluation: true, moreOrgs: true })
    const select = await screen.findByRole('combobox', { name: 'Org' })
    expect(within(select).getAllByRole('option').length).toBeGreaterThan(1)
  })

  it('reads in Japanese with the glossary’s words', async () => {
    renderApp({ path: '/evaluation', evaluation: true, locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 2, name: 'Appチーム' })).toBeInTheDocument()
    expect(screen.getAllByText('マージしたプルリクエスト').length).toBeGreaterThan(0)
    expect(screen.getByRole('link', { name: '評価' })).toBeInTheDocument()
  })

  it('is in the menu of the Results pages, after the costs', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const links = within(await screen.findByRole('navigation', { name: 'Results pages' })).getAllByRole('link').map((link) => link.textContent)
    expect(links).toEqual(['Costs', 'Evaluation'])
  })
})
