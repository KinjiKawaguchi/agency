import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
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
    expect(within(legend).getAllByRole('listitem').map((item) => item.textContent)).toEqual(['New feature', 'Fix', 'Refactor', 'Docs', 'Test', 'Chore', 'Other', 'Revert'])
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
    await waitFor(() => expect(screen.getAllByRole('alert')).toHaveLength(2))
    const texts = screen.getAllByRole('alert').map((alert) => alert.textContent)
    expect(texts).toContain('A team could not report its numbers right now: The evaluation could not be read.')
    expect(texts).toContain('A team could not report its effort right now: The evaluation could not be read.')
    expect(screen.getByRole('heading', { level: 1, name: 'Evaluation' })).toBeInTheDocument()
  })

  it('draws the teams that answered and says why one could not, instead of blanking the screen', async () => {
    renderApp({ path: '/evaluation', evaluation: true, evaluationUnavailableTeam: 'site' })
    const app = await section('App team')
    expect(app.getByRole('group', { name: 'Merged pull requests per day' })).toBeInTheDocument()
    const speed = (await screen.findAllByRole('heading', { level: 2, name: 'Site team' }))[0] as HTMLElement
    expect(within(speed.closest('section') as HTMLElement).getByText('This team could not report its numbers: Its node did not answer.')).toBeInTheDocument()
    const effort = (await screen.findAllByRole('heading', { level: 3, name: 'Site team' }))[0] as HTMLElement
    expect(within(effort.closest('section') as HTMLElement).getByText('This team could not report its numbers: Its node did not answer.')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
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

describe('evaluation: where effort goes', () => {
  const effortOf = async (team: string) => {
    const heading = (await screen.findAllByRole('heading', { level: 3, name: `${team} team` }))[0] as HTMLElement
    return within(heading.closest('section') as HTMLElement)
  }

  it('shows each work type of a team with its share, and the plan beside it', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await effortOf('App')
    const rows = app.getAllByRole('listitem').filter((item) => item.classList.contains('ev-alloc-row'))
    expect(rows.map((row) => row.querySelector('.ev-alloc-name')?.textContent)).toEqual(['New feature', 'Improvement', 'Maintenance', 'Tech debt', 'Incident', 'Unclassified'])
    expect(rows[0]?.textContent).toMatch(/intended 50%/)
    expect(rows[5]?.textContent).not.toMatch(/intended/)
    expect(rows[0]?.querySelector('.ev-alloc-mark')).not.toBeNull()
    expect(rows[5]?.querySelector('.ev-alloc-mark')).toBeNull()
  })

  it('says unclassified work is left unclassified, not guessed', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    expect(await within((await effortOf('App')).getByText(/stays unclassified/).parentElement as HTMLElement).findByText(/It is not guessed\./)).toBeInTheDocument()
  })

  it('measures the last seven days against the weekly budget', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await effortOf('App')
    expect(app.getByRole('img', { name: /spent of a weekly budget of \$100$/ })).toBeInTheDocument()
    expect(app.getByText(/over$/)).toBeInTheDocument()
    expect(app.getByText(/over$/)).toBeInTheDocument()
  })

  it('says a team has no plan, and compares nothing', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const site = await effortOf('Site')
    expect(site.getByText('The org has stated no plan for this team, so there is nothing to compare with.')).toBeInTheDocument()
    expect(site.queryByText(/intended/)).toBeNull()
    expect(site.queryByRole('img', { name: /weekly budget/ })).toBeNull()
  })

  it('draws cost by day for every work type', async () => {
    renderApp({ path: '/evaluation', evaluation: true })
    const app = await effortOf('App')
    expect(app.getByRole('group', { name: 'Cost by day' }).querySelectorAll('.ev-col')).toHaveLength(14)
  })

  it('says so, in the place of the numbers, when the node does not report effort', async () => {
    renderApp({ path: '/evaluation' })
    expect(await screen.findByText('This node does not report effort by work type yet.')).toBeInTheDocument()
  })

  it('names the route an agent reads for the effort', async () => {
    renderApp({ path: '/evaluation?days=30', evaluation: true })
    expect(await screen.findByText(/GET \/api\/orgs\/acme\/effort\?days=30/)).toBeInTheDocument()
  })

  /** A client whose node leaves out `incident` from the effort, and `other` from the merged counts, as an older node might. */
  const clientWithGaps = () => {
    const node = createMockNode({ evaluation: true })
    return createApiClient(async (request) => {
      const reply = await node.handle(request)
      const path = new URL(request.url, 'http://mock.local').pathname
      if (!path.endsWith('/effort') && !path.endsWith('/evaluation')) return reply
      const body = JSON.parse(await reply.text()) as { teams: Array<Record<string, any>> }
      for (const team of body.teams) {
        if (team.byType !== undefined) delete team.byType.incident
        for (const day of team.byDay) {
          if (day.costUsd !== null && typeof day.costUsd === 'object') delete day.costUsd.debt
          if (day.merged !== undefined) delete day.merged.other
        }
        if (team.summary !== undefined) delete team.summary.merged.other
      }
      return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } })
    })
  }

  it('still draws both sections when the node leaves a type out of a total or of a day', async () => {
    renderApp({ path: '/evaluation', client: clientWithGaps() })
    const app = await effortOf('App')
    expect(app.getAllByRole('listitem').filter((item) => item.classList.contains('ev-alloc-row'))).toHaveLength(6)
    expect(app.getByRole('group', { name: 'Cost by day' }).querySelectorAll('.ev-col')).toHaveLength(14)
    expect(await section('App team')).toBeDefined()
    expect(screen.queryByRole('alert')).toBeNull()
  })
})
