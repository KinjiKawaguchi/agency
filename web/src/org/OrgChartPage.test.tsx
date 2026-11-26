import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { sampleTeamView } from '../api/teamview'
import { renderApp } from '../test/render'

/** The chart box of a kind and name, such as ('Department', 'Design'). Its text contains both, one after the other. */
const box = (kind: string, name: string): Promise<HTMLElement> =>
  waitFor(() => {
    const found = screen.getAllByRole('link').find((link) => link.textContent?.includes(`${kind}${name}`))
    if (found === undefined) throw new Error(`No ${kind} ${name}`)
    return found
  })

describe('org chart', () => {
  it('draws the organization with its departments, teams, and people', async () => {
    renderApp({ path: '/org' })
    const chart = within(await screen.findByRole('group', { name: 'Organization chart' }))
    expect(chart.getByText('Acme')).toBeInTheDocument()
    expect(await box('Department', 'Engineering')).toHaveTextContent('Led by Alice')
    expect(await box('Department', 'Platform')).toHaveTextContent('Led by Bob')
    expect(await box('Department', 'Design')).toHaveTextContent('Led by Carol')
    expect(chart.getByText('Carol')).toBeInTheDocument()
    expect(chart.getByText('Answers only')).toBeInTheDocument()
  })

  it('marks the viewer', async () => {
    renderApp({ path: '/org' })
    const you = (await screen.findByText('You')).closest('.oc-node')
    expect(you).toHaveTextContent('Alice')
  })

  it('shows the agents under their team with the one waiting for the viewer', async () => {
    renderApp({ path: '/org' })
    const team = await box('Team', 'App')
    expect(team).toHaveTextContent('4 agents, 2 working')
    expect(team).toHaveTextContent('1 waiting for you')
    expect(await box('Backend', 'Agent')).toHaveTextContent('Waiting for your answer')
    expect(await box('Lead', 'Lead (agent)')).toHaveTextContent('Reviewing app#12')
  })

  it('tells a department how many are waiting below it', async () => {
    renderApp({ path: '/org' })
    expect(await box('Department', 'Engineering')).toHaveTextContent('1 waiting for you')
    expect(await box('Department', 'Design')).not.toHaveTextContent('waiting for you')
  })

  it('focuses on a department and comes back to the whole organization', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/org' })
    await user.click(await box('Department', 'Engineering'))
    expect(await screen.findByRole('link', { name: 'Back to the whole organization' })).toBeInTheDocument()
    expect(await box('Department', 'Platform')).toBeInTheDocument()
    expect(screen.queryByText('Design')).not.toBeInTheDocument()
    await user.click(screen.getByRole('link', { name: 'Back to the whole organization' }))
    expect(await box('Department', 'Design')).toBeInTheDocument()
  })

  it('shows the whole organization for a department that does not exist', async () => {
    renderApp({ path: '/org?unit=nowhere' })
    expect(await box('Department', 'Design')).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Back to the whole organization' })).not.toBeInTheDocument()
  })

  describe('across every org', () => {
    it('draws every org of the account in one chart when it has several and none is named', async () => {
      renderApp({ path: '/org', moreOrgs: true })
      const chart = within(await screen.findByRole('group', { name: 'Chart of every organization' }))
      expect(screen.getByRole('heading', { level: 1, name: 'All your organizations' })).toBeInTheDocument()
      expect(await within(chart.getByText('Account').closest('.oc-node') as HTMLElement).findByText('Alice')).toBeInTheDocument()
      for (const name of ['Acme', 'Lab', 'Club']) expect(await box('Organization', name)).toBeInTheDocument()
      expect(chart.getByText('Research')).toBeInTheDocument()
    })

    it('draws the one org as it always did when the account has only one', async () => {
      renderApp({ path: '/org' })
      expect(await screen.findByRole('group', { name: 'Organization chart' })).toBeInTheDocument()
      expect(screen.queryByRole('group', { name: 'Chart of every organization' })).toBeNull()
      expect(screen.queryByRole('combobox', { name: 'Organization' })).toBeNull()
    })

    it('shows the signals of the overview on each team and rolls the waiting requests up to the department and the org', async () => {
      renderApp({ path: '/org', moreOrgs: true })
      // The node lists three requests as the viewer's in the sample team.
      await waitFor(async () => expect(await box('Organization', 'Acme')).toHaveTextContent('3 waiting for you'))
      await waitFor(async () => expect(await box('Team', 'App')).toHaveTextContent('3 waiting for you'))
      await waitFor(async () => expect(await box('Department', 'Engineering')).toHaveTextContent('3 waiting for you'))
      await waitFor(async () => expect(await box('Team', 'Research')).toHaveTextContent('Stuck'))
    })

    it('says a team could not be read in its place', async () => {
      renderApp({ path: '/org', moreOrgs: true })
      expect(await box('Team', 'Shop')).toHaveTextContent('Could not read this team')
    })

    it('shows participants as faces in the team’s box, six at most, and says how many more', async () => {
      const extra = ['a', 'b', 'c', 'd'].map((id) => ({ id: `extra-${id}`, label: `Extra ${id}`, kind: 'agent' as const, role: 'member' as const, state: 'idle' as const, status: 'Idle', holding: [] }))
      renderApp({ path: '/org', moreOrgs: true, view: { ...sampleTeamView, people: [...sampleTeamView.people, ...extra] } })
      await waitFor(async () => expect((await box('Team', 'App')).querySelectorAll('.oc-faces .face')).toHaveLength(6))
      expect((await box('Team', 'App')).querySelector('.oc-more')).toHaveTextContent(/^\+\d+$/)
      expect((await box('Team', 'Site')).querySelector('.oc-more')).toBeNull()
    })

    it('opens one org’s own chart from its box, and offers all of them first in the menu', async () => {
      const user = userEvent.setup()
      renderApp({ path: '/org', moreOrgs: true })
      const menu = await screen.findByRole('combobox', { name: 'Organization' })
      expect(within(menu).getAllByRole('option').map((option) => option.textContent)).toEqual(['All organizations', 'Acme', 'Lab', 'Club'])
      await user.click(await box('Organization', 'Lab'))
      expect(await screen.findByRole('group', { name: 'Organization chart' })).toBeInTheDocument()
      expect(screen.getByRole('heading', { level: 1, name: 'Lab organization' })).toBeInTheDocument()
      await user.selectOptions(screen.getByRole('combobox', { name: 'Organization' }), 'all')
      expect(await screen.findByRole('group', { name: 'Chart of every organization' })).toBeInTheDocument()
    })

    it('draws the orgs it could read and says which it could not, instead of leaving the chart empty', async () => {
      const node = createMockNode({ moreOrgs: true })
      const client = createApiClient(async (request) => (new URL(request.url, 'http://mock.local').pathname === '/api/orgs/club' ? new Response(JSON.stringify({ error: 'The org’s node did not answer.' }), { status: 503, headers: { 'content-type': 'application/json' } }) : node.handle(request)))
      renderApp({ path: '/org', moreOrgs: true, client })
      await waitFor(async () => expect(await box('Organization', 'Club')).toHaveTextContent('Could not be read'))
      expect(await box('Organization', 'Acme')).not.toHaveTextContent('Could not be read')
      expect(await box('Department', 'Engineering')).toBeInTheDocument()
    })

    it('reads in Japanese', async () => {
      renderApp({ path: '/org', moreOrgs: true, locale: 'ja' })
      expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent(/すべて/)
    })
  })
})
