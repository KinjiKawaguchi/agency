import { act, renderHook, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import { useReducedMotion } from './useReducedMotion'

describe('the live page', () => {
  it('is its own page, marked experimental, in the results menu', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/overview', activity: true })
    await user.click(await screen.findByRole('link', { name: 'Results' }))
    expect(await screen.findByRole('link', { name: 'Live' })).toHaveAttribute('href', '/live')
    renderApp({ path: '/live', activity: true })
    expect(await screen.findByRole('heading', { level: 1, name: 'Live' })).toBeInTheDocument()
    expect(screen.getByText('Experimental')).toBeInTheDocument()
  })

  it('shows what is happening, newest first: messages between agents and what the board did', async () => {
    renderApp({ path: '/live', activity: true })
    const feed = await screen.findByRole('list', { name: 'What is happening' })
    await waitFor(() => expect(within(feed).getAllByRole('listitem').length).toBeGreaterThan(10))
    const rows = within(feed).getAllByRole('listitem')
    expect(rows.some((row) => /lead → |→ lead/.test(row.textContent ?? ''))).toBe(true)
    const times = rows.map((row) => row.querySelector('.lv-time')?.textContent)
    expect(times.every((time) => time !== undefined && time !== '')).toBe(true)
    expect(feed.querySelector('.lv-kind-statusChanged, .lv-kind-prMerged, .lv-kind-prOpened, .lv-kind-itemAdded')).not.toBeNull()
  })

  it('draws the agents as points and the messages between them as lines', async () => {
    renderApp({ path: '/live', activity: true })
    const graph = await screen.findByRole('img', { name: /Messages between agents/ })
    await waitFor(() => expect(graph.querySelectorAll('line.lv-edge').length).toBeGreaterThan(0))
    expect(Array.from(graph.querySelectorAll('.lv-node title')).map((title) => title.textContent)).toEqual(expect.arrayContaining(['Lead, app team', 'Backend, app team']))
    // A line joins two agents of the same team: every point has a place inside the picture.
    for (const node of Array.from(graph.querySelectorAll('.lv-node'))) expect(node.getAttribute('transform')).toMatch(/^translate\(-?[\d.]+ -?[\d.]+\)$/)
  })

  it('shows what each agent works on now, with the epic of that work', async () => {
    renderApp({ path: '/live', activity: true })
    const board = await screen.findByRole('region', { name: 'Working on now' })
    expect(within(board).getByText('app#12')).toBeInTheDocument()
    expect(within(board).getAllByText(/in Persistent sessions \(app#10\)/).length).toBeGreaterThan(0)
  })

  it('shows the throughput: merges in the last hour and in bins of five minutes', async () => {
    renderApp({ path: '/live', activity: true })
    const bins = await screen.findByRole('img', { name: 'Merges in each 5 minutes of the last hour' })
    expect(bins.querySelectorAll('.lv-bin')).toHaveLength(12)
    expect(screen.getByText('Merged in the last hour')).toBeInTheDocument()
  })

  it('says so, and keeps the rest, when the node does not report activity yet', async () => {
    renderApp({ path: '/live' })
    expect(await screen.findByText(/This node does not report activity yet/)).toBeInTheDocument()
    expect(within(await screen.findByRole('region', { name: 'Working on now' })).getByText('app#12')).toBeInTheDocument()
    expect(screen.queryByRole('alert')).toBeNull()
  })

  it('says a feed is late, in its place, when a team’s activity cannot be read', async () => {
    renderApp({ path: '/live', activity: true, activityFailsWith: 503 })
    expect(await screen.findByText(/The feed of .* is late\./)).toBeInTheDocument()
    expect(screen.getByRole('region', { name: 'Working on now' })).toBeInTheDocument()
  })

  it('reads in Japanese', async () => {
    renderApp({ path: '/live', activity: true, locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 1, name: 'ライブ' })).toBeInTheDocument()
    expect(screen.getByText('実験中')).toBeInTheDocument()
    expect(await screen.findByRole('list', { name: '今起きていること' })).toBeInTheDocument()
  })
})

describe('the wish for less motion', () => {
  const originalMatchMedia = window.matchMedia

  afterEach(() => void (window.matchMedia = originalMatchMedia))

  it('is read, and followed while the page is open', () => {
    let listener: (() => void) | undefined
    let matches = true
    window.matchMedia = ((query: string) => ({
      get matches() {
        return matches
      },
      media: query,
      addEventListener: (_: string, fn: () => void) => void (listener = fn),
      removeEventListener: () => undefined,
    })) as unknown as typeof window.matchMedia
    const { result } = renderHook(() => useReducedMotion())
    expect(result.current).toBe(true)
    matches = false
    act(() => listener?.())
    expect(result.current).toBe(false)
  })

  it('draws nothing that travels or arrives with a stir where it is asked for', async () => {
    window.matchMedia = ((query: string) => ({ matches: query.includes('reduce'), media: query, addEventListener: () => undefined, removeEventListener: () => undefined })) as unknown as typeof window.matchMedia
    renderApp({ path: '/live', activity: true })
    const graph = await screen.findByRole('img', { name: /Messages between agents/ })
    await waitFor(() => expect(graph.querySelectorAll('line.lv-edge').length).toBeGreaterThan(0))
    expect(graph.querySelectorAll('.lv-packet')).toHaveLength(0)
    expect(document.querySelector('.lv-new')).toBeNull()
  })
})
