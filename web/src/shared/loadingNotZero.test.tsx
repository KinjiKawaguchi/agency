import { act, screen, waitFor, within } from '@testing-library/react'
import { flakyNode } from '../test/flakyNode'
import { heldNode } from '../test/heldNode'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

// While the node has not answered, a screen shows that it is loading, never a value that looks like an answer
// (a 0, an empty list, "nothing to decide"): nobody could tell loading from really 0. Each test holds one read, looks
// at the screen, releases it, and looks again.

const TEAM_VIEW = /\/teams\/app\/view$/

const waiting = (main: HTMLElement): string => within(main).getByRole('link', { name: /Decisions/ }).textContent ?? ''

async function held(path: string, holds: ReadonlyArray<string | RegExp>, options: Parameters<typeof createTestNode>[0] = {}) {
  const node = createTestNode(options)
  const { client, release } = heldNode(node, holds)
  renderApp({ path, node, client, enrolled: true })
  return { release: (match?: string | RegExp) => act(() => release(match)) }
}

describe('a screen before the node has answered', () => {
  it('has a placeholder for the count on the Decisions tab, then the number', async () => {
    const { release } = await held('/overview', [TEAM_VIEW])
    const main = await screen.findByRole('navigation', { name: 'Main menu' })
    await waitFor(() => expect(main.querySelector('.nav-count.skeleton-count')).not.toBeNull())
    expect(waiting(main)).toBe('Decisions')
    expect(document.title).toBe('Agency')
    await release()
    await waitFor(() => expect(waiting(main)).toMatch(/Decisions\d+/))
    expect(main.querySelector('.skeleton-count')).toBeNull()
    expect(document.title).toMatch(/^\(\d+\) /)
  })

  it('does not say nothing needs a decision until the teams have answered', async () => {
    const { release } = await held('/desk', [TEAM_VIEW])
    await screen.findByText(/^Good|^Hello|Alice/)
    await waitFor(() => expect(document.querySelector('.briefing-line .skeleton')).not.toBeNull())
    expect(screen.queryByText(/Nothing needs your decision/)).not.toBeInTheDocument()
    expect(document.querySelector('.briefing-sub .skeleton')).not.toBeNull()
    await release()
    await waitFor(() => expect(document.querySelector('.briefing-line .skeleton')).toBeNull())
    expect(document.querySelector('.briefing-line')?.textContent).not.toBe('')
  })

  it('has a placeholder, not an empty list, for the decisions made lately', async () => {
    const { release } = await held('/desk', ['/api/decisions'])
    await screen.findByRole('navigation', { name: 'Main menu' })
    // The pile has its rows (the teams answered), so the only placeholder box left is the one of the decisions.
    await screen.findByRole('listbox')
    expect(screen.queryByRole('region', { name: /Recently decided/ })).not.toBeInTheDocument()
    await waitFor(() => expect(document.querySelectorAll('[aria-busy="true"] .skeleton.box').length).toBe(1))
    await release()
    expect(await screen.findByRole('region', { name: /Recently decided/ })).toBeInTheDocument()
  })

  it('has a placeholder for what is waiting in the overview lead', async () => {
    const { release } = await held('/overview', [TEAM_VIEW])
    await waitFor(() => expect(document.querySelector('.ov-lead .skeleton')).not.toBeNull())
    expect(screen.queryByText(/waits? for you|Nothing waits/)).not.toBeInTheDocument()
    await release()
    await waitFor(() => expect(document.querySelector('.ov-lead .skeleton')).toBeNull())
    expect(document.querySelector('.ov-lead')?.textContent).not.toBe('')
  })

  it('draws no count for a team or a department while a team below is read', async () => {
    const { release } = await held('/org', [TEAM_VIEW])
    await screen.findByRole('heading', { level: 1 })
    await waitFor(() => expect(document.querySelectorAll('.oc-node .skeleton').length).toBeGreaterThan(0))
    expect(screen.queryByText(/waits? for you/)).not.toBeInTheDocument()
    // The org's own box is a placeholder too, not a count that sums only the teams that answered.
    expect(document.querySelector('.oc-node.org .skeleton, .oc-node.department .skeleton')).not.toBeNull()
    await release()
    await waitFor(() => expect(screen.getAllByText(/waits? for you/).length).toBeGreaterThan(0))
  })

  it('does not say the org has no plan for a team while its definition is read', async () => {
    const { release } = await held('/evaluation', [/^\/api\/orgs\/acme$/], { evaluation: true })
    await screen.findByText(/Where does the effort go|effort/i)
    await waitFor(() => expect(document.querySelector('.ev-team .card .skeleton')).not.toBeNull())
    expect(screen.queryByText(/stated no plan/)).not.toBeInTheDocument()
    await release()
    await waitFor(() => expect(document.querySelector('.ev-team .card .skeleton')).toBeNull())
  })

  it('has placeholders for the live numbers until the activity has been read, then the numbers', async () => {
    const { release } = await held('/live', ['/activity'], { activity: true, evaluation: true })
    await waitFor(() => expect(document.querySelectorAll('.lv-tile-value .skeleton').length).toBeGreaterThan(1))
    expect(document.querySelector('.lv-bins')).toBeNull()
    expect(screen.queryByText('Nothing in the last hour yet.')).not.toBeInTheDocument()
    await release()
    await waitFor(() => expect(document.querySelectorAll('.lv-tile-value .skeleton').length).toBe(0))
    expect(document.querySelector('.lv-bins')).not.toBeNull()
  })

  it('has a placeholder for a team of the costs page that has not answered', async () => {
    const { release } = await held('/costs', [TEAM_VIEW])
    await screen.findByRole('heading', { level: 1 })
    await waitFor(() => expect(document.querySelector('[aria-busy="true"] .skeleton.box')).not.toBeNull())
    await release()
    await waitFor(() => expect(document.querySelector('[aria-busy="true"] .skeleton.box')).toBeNull())
  })

  it('has a placeholder for the lead time and the day until the evaluation has answered, then the numbers', async () => {
    const { release } = await held('/live', ['/evaluation'], { activity: true, evaluation: true })
    await screen.findByRole('heading', { level: 1 })
    const tiles = () => [...document.querySelectorAll('.lv-tile')]
    await waitFor(() => expect(tiles().filter((tile) => tile.querySelector('.skeleton') !== null).length).toBeGreaterThanOrEqual(2))
    const lead = () => tiles().find((tile) => tile.textContent?.includes('Lead time'))
    expect(lead()?.querySelector('.skeleton')).not.toBeNull()
    await release()
    await waitFor(() => expect(lead()?.querySelector('.skeleton')).toBeNull())
    expect(lead()?.textContent).toMatch(/\d/)
  })

  it('shows a dash, not a figure, for the lead time and the day when the evaluation could not be read', async () => {
    renderApp({ path: '/live', activity: true, evaluation: true, evaluationFailsWith: 500 })
    await waitFor(() => expect(document.querySelectorAll('.lv-tile-value').length).toBe(3), { timeout: 4_000 })
    await waitFor(() => expect(document.querySelectorAll('.lv-tile-value .skeleton').length).toBe(0), { timeout: 4_000 })
    const values = [...document.querySelectorAll('.lv-tile-value')].map((one) => one.textContent)
    expect(values[1], 'the day').toBe('—')
    expect(values[2], 'the lead time').toBe('—')
  })

  it('shows a dash and not 0, and says the feed could not be read, for a team that was never read', async () => {
    renderApp({ path: '/live', activity: true, activityFailsWith: 500, evaluation: true })
    await screen.findByText('The feed could not be read yet.', undefined, { timeout: 8_000 })
    const values = [...document.querySelectorAll('.lv-tile-value')].map((one) => one.textContent)
    expect(values.slice(0, 2)).toEqual(['—', '—'])
    expect(document.querySelector('.lv-bins')).toBeNull()
    expect(screen.queryByText('Nothing in the last hour yet.')).not.toBeInTheDocument()
  })

  it('has a placeholder for the week cost of a team card until its usage has answered, then the cost', async () => {
    const { release } = await held('/overview', ['/usage'], { usage: true })
    await waitFor(() => expect(document.querySelector('.ov-teams .ov-line .skeleton')).not.toBeNull())
    expect(screen.queryByText(/this week|API price/i)).not.toBeInTheDocument()
    await release()
    await waitFor(() => expect(document.querySelector('.ov-teams .ov-line .skeleton')).toBeNull())
  })

  it('says some teams could not be read, on the org and the department, and not that nothing waits', async () => {
    renderApp({ path: '/org', client: flakyNode([{ match: '/teams/site/view', status: 500 }]).client })
    await waitFor(() => expect(screen.getAllByText('Some teams could not be read').length).toBeGreaterThan(0), { timeout: 8_000 })
  })

  it('does not let the overview lead pass for the whole when a team could not be read', async () => {
    renderApp({ path: '/overview', client: flakyNode([{ match: '/teams/site/view', status: 500 }]).client })
    await screen.findByText(/team could not be read\./, undefined, { timeout: 8_000 })
    expect(document.querySelector('.ov-lead')?.textContent).toMatch(/that answered/)
  })
})

