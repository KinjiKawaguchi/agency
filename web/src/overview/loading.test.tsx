import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { flakyNode } from '../test/flakyNode'
import { renderApp } from '../test/render'

/** The screens that read team views treat "not ready yet" as loading: a skeleton while it is tried again, an error only when it keeps failing. */
describe('a team view that is not ready yet', () => {
  it('is a skeleton, not an error, until the node answers', async () => {
    const { client, reads } = flakyNode([{ match: '/teams/app/view', failures: 2 }])
    renderApp({ path: '/overview', client, retryDelays: [0, 0, 0] })
    expect(await screen.findByText('Loading…')).toBeInTheDocument()
    expect(await screen.findByRole('heading', { level: 3, name: 'App' })).toBeInTheDocument()
    expect(reads('/teams/app/view')).toBe(3)
    expect(screen.queryByText(/Could not load/)).toBeNull()
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('holds nothing else back while it is tried again: the other teams are drawn, and its card is a skeleton', async () => {
    const { client } = flakyNode([{ match: '/teams/site/view' }])
    // A wait of a minute before the first retry: the site team stays "not ready" for the whole test.
    renderApp({ path: '/overview', client, retryDelays: [60_000] })
    expect(await screen.findByRole('heading', { level: 3, name: 'App' })).toBeInTheDocument()
    const site = screen.getByRole('article', { name: 'acme/site' })
    expect(within(site).getByText('Loading…')).toBeInTheDocument()
    expect(within(site).queryByRole('button', { name: 'Try again' })).toBeNull()
  })

  it('is shown in its place, with why and when, when it keeps failing, and the others are drawn', async () => {
    const { client } = flakyNode([{ match: '/teams/site/view' }])
    renderApp({ path: '/overview', client, retryDelays: [0, 0] })
    const site = (await screen.findByRole('article', { name: 'acme/site' }))
    expect(await within(site).findByText(/Could not load this team: The board has not been read yet/)).toBeInTheDocument()
    expect(within(site).getByText(/Trying again in \d+ s\./)).toBeInTheDocument()
    expect(within(site).getByRole('button', { name: 'Try again' })).toBeInTheDocument()
    expect(await screen.findByRole('heading', { level: 3, name: 'App' })).toBeInTheDocument()
  })

  it('is not tried again when the node refuses it for good', async () => {
    const { client, reads } = flakyNode([{ match: '/teams/site/view', status: 404 }])
    renderApp({ path: '/overview', client, retryDelays: [0, 0, 0] })
    const site = await screen.findByRole('article', { name: 'acme/site' })
    expect(await within(site).findByText(/Could not load this team/)).toBeInTheDocument()
    expect(reads('/teams/site/view')).toBe(1)
  })

  it('draws the org chart once it is ready, and marks the team that keeps failing', async () => {
    const { client } = flakyNode([{ match: '/teams/app/view', failures: 1 }, { match: '/teams/site/view' }])
    renderApp({ path: '/org', client, retryDelays: [0, 0] })
    expect(await screen.findByText('Loading…')).toBeInTheDocument()
    expect(await screen.findByRole('group', { name: 'Organization chart' })).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
    expect(await screen.findByText('Could not read this team')).toBeInTheDocument()
  })

  it('says in the org chart’s place what failed when the org’s definition keeps failing, and recovers on request', async () => {
    const user = userEvent.setup()
    const { client } = flakyNode([{ match: '/api/orgs/acme', exact: true, failures: 3 }])
    renderApp({ path: '/org', client, retryDelays: [0] })
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Could not load the org chart: The board has not been read yet')
    expect(alert).toHaveTextContent('Trying again in')
    await user.click(within(alert).getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('group', { name: 'Organization chart' })).toBeInTheDocument()
  })

  it('shows a team that keeps failing in participants and work, with the others', async () => {
    const { client } = flakyNode([{ match: '/teams/site/view' }])
    renderApp({ path: '/people', client, retryDelays: [0] })
    expect(await screen.findByRole('heading', { level: 2, name: 'App team' })).toBeInTheDocument()
    const site = (await screen.findByRole('heading', { level: 2, name: 'Site team' })).closest('section') as HTMLElement
    expect(await within(site).findByText(/Could not load this team/)).toBeInTheDocument()
  })

  it('does not blank the page when the account itself cannot be read: the region says so, the menu stays', async () => {
    const { client } = flakyNode([{ match: '/api/me', exact: true }])
    renderApp({ path: '/overview', client, retryDelays: [0] })
    expect(await screen.findByRole('alert')).toHaveTextContent('Could not load the overview')
    expect(screen.getByRole('link', { name: 'Overview' })).toBeInTheDocument()
  })
})
