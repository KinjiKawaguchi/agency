import { screen, waitFor, within } from '@testing-library/react'
import { renderApp } from '../test/render'

const card = async (org: string, team: string) => within(await screen.findByRole('article', { name: `${org}/${team}` }))

describe('overview', () => {
  it('lists every org and team of the account, and says how many requests wait for the viewer and in how many teams', async () => {
    renderApp({ path: '/overview', moreOrgs: true, usage: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Overview' })).toBeInTheDocument()
    await waitFor(() => expect(document.querySelectorAll('article.ov-team')).toHaveLength(6))
    await waitFor(() => expect(screen.getByText(/requests wait for you, in 2 teams\./)).toBeInTheDocument())
    expect(screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent)).toEqual(['Acme', 'Lab', 'Club', 'Usage limits'])
  })

  it('puts a team that waits on the viewer first in its org, and links it to the desk filtered to that team', async () => {
    renderApp({ path: '/overview', moreOrgs: true })
    const lab = await screen.findByRole('region', { name: 'Lab' })
    await waitFor(() => expect(within(lab).getAllByRole('article').map((one) => one.getAttribute('aria-label'))).toEqual(['lab/notebook', 'lab/research']))
    const link = within(lab).getByRole('link', { name: '1 request waits for you' })
    expect(link.getAttribute('href')).toMatch(/^\/desk\?team=lab(%2F|\/)notebook$/)
  })

  it('says why a team is stuck, as the team reports it', async () => {
    renderApp({ path: '/overview', moreOrgs: true })
    const research = await card('lab', 'research')
    expect(await research.findByText('Stuck')).toBeInTheDocument()
    expect(research.getByText('3 blocked items')).toBeInTheDocument()
    expect(research.getByText('Needs a decision (2)')).toBeInTheDocument()
    expect(research.getByText('Waiting on an outside party (1)')).toBeInTheDocument()
    expect(research.getByText('Nothing waits for you.')).toBeInTheDocument()
  })

  it('counts the participants who are working out of all of them, the viewer left out', async () => {
    renderApp({ path: '/overview', moreOrgs: true })
    expect(await (await card('club', 'garden')).findByText('2 of 3 participants working')).toBeInTheDocument()
  })

  it('shows how old a copy is and why it could not be renewed', async () => {
    renderApp({ path: '/overview', moreOrgs: true })
    const notebook = await card('lab', 'notebook')
    expect(await notebook.findByText(/^Data from .*The team’s node could not be reached: The team’s node did not answer\.$/)).toBeInTheDocument()
  })

  it('shows a team that cannot be read in its place, and the rest of the page stays', async () => {
    renderApp({ path: '/overview', moreOrgs: true })
    const shop = await card('club', 'shop')
    expect(await shop.findByText('Could not load this team: The team’s node has not been reached yet.')).toBeInTheDocument()
    expect(shop.getByText('Not loaded')).toBeInTheDocument()
    expect(await (await card('club', 'garden')).findByText('2 of 3 participants working')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('reads the week’s API-priced cost of a team, and skips a team whose node is known not to answer', async () => {
    renderApp({ path: '/overview', moreOrgs: true, usage: true })
    expect(await (await card('club', 'garden')).findByText(/this week, API-priced$/)).toBeInTheDocument()
    expect((await card('lab', 'notebook')).queryByText(/this week, API-priced$/)).not.toBeInTheDocument()
  })

  it('says a team does not report its token use when its runtime answers 404', async () => {
    renderApp({ path: '/overview', moreOrgs: true })
    expect(await (await card('club', 'garden')).findByText('Token use is not reported.')).toBeInTheDocument()
  })

  it('lists every funding source with its windows, what is left, and when they reset', async () => {
    renderApp({ path: '/overview' })
    const funding = await screen.findByRole('region', { name: 'Usage limits' })
    expect((await within(funding).findAllByRole('article')).length).toBeGreaterThan(1)
    expect(within(funding).getAllByText(/% left$/).length).toBeGreaterThan(1)
    expect(within(funding).getAllByText(/^Resets /).length).toBeGreaterThan(1)
  })

  it('reads in Japanese', async () => {
    renderApp({ path: '/overview', moreOrgs: true, locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 1, name: '全体の状況' })).toBeInTheDocument()
    expect(await (await card('club', 'garden')).findByText('参加者 3 人のうち 2 人が作業中')).toBeInTheDocument()
    expect(await (await card('club', 'shop')).findByText(/^このチームを読み込めませんでした: /)).toBeInTheDocument()
  })
})
