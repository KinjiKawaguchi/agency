import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { heldNode } from '../test/heldNode'
import { renderApp } from '../test/render'

const TITLE = 'Response time of this node'
const section = async () => within((await screen.findByRole('heading', { level: 2, name: TITLE })).closest('section') as HTMLElement)
const open = (options: Parameters<typeof createMockNode>[0] = {}, extra: Parameters<typeof renderApp>[0] = {}) => renderApp({ path: '/evaluation', evaluation: true, node: createMockNode({ evaluation: true, ...options }), ...extra })
const row = (scope: ReturnType<typeof within>, name: string) => scope.getByRole('rowheader', { name }).closest('tr') as HTMLElement

describe('the node’s own response time, on the evaluation page', () => {
  it('is not drawn for anyone but the owner: no heading, no alert, nothing that says there is something', async () => {
    const node = createMockNode({ evaluation: true })
    const answers: number[] = []
    const client = createApiClient(async (request) => {
      const answer = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname === '/api/metrics') answers.push(answer.status)
      return answer
    })
    renderApp({ path: '/evaluation', node, client })
    await screen.findByRole('heading', { level: 1, name: 'Evaluation' })
    await waitFor(() => expect(answers).toEqual([403]))
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(screen.queryByText(TITLE)).not.toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(screen.queryByText(/in-node read/i)).not.toBeInTheDocument()
  })

  it('draws nothing of numbers before the node has answered, only the heading and a skeleton', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'usual' })
    const { client, release } = heldNode(node, ['/api/metrics'])
    renderApp({ path: '/evaluation', node, client })
    const found = await section()
    expect(found.queryByRole('table')).not.toBeInTheDocument()
    expect(found.queryByText(/Requests|Within the budget|Over the budget|No in-node read|Nothing was measured/)).not.toBeInTheDocument()
    expect(document.querySelector('.mt-section .skeleton')).not.toBeNull()
    release()
    expect(await found.findAllByText('Requests')).toHaveLength(2)
    expect(document.querySelector('.mt-section .skeleton')).toBeNull()
  })

  it('says one line when the node does not measure, and an alert when it fails', async () => {
    open({ metrics: 'absent' })
    expect(await (await section()).findByText('This node does not measure its response time yet.')).toBeInTheDocument()
  })

  it('says what failed when the node cannot give the measurements, and still draws the rest of the page', async () => {
    const { unmount } = open({ metricsFailWith: 503 })
    expect(await (await section()).findByRole('alert')).toHaveTextContent('could not give its measurements')
    expect(screen.getByRole('heading', { level: 2, name: 'App team' })).toBeInTheDocument()
    unmount()
  })

  it('says that nothing was measured, which is not 0 ms: no table, no percentile, no budget verdict', async () => {
    open({ metrics: 'empty' })
    const found = await section()
    expect(await found.findByText('Nothing was measured in this period yet.')).toBeInTheDocument()
    expect(found.queryByRole('table')).not.toBeInTheDocument()
    expect(found.queryByText(/0 ms/)).not.toBeInTheDocument()
    expect(found.queryByText(/Within the budget|Over the budget/)).not.toBeInTheDocument()
  })

  it('puts the in-node reads against the budget: within it when their p95 is at most the bound', async () => {
    open({ metrics: 'usual' })
    const found = await section()
    expect(await found.findByText(/Within the budget: p95 ≤ 50 ms\./)).toBeInTheDocument()
    expect(found.getByText(/An in-node read is one the node answers from its own state/)).toBeInTheDocument()
    expect(found.queryByText('Reads over the budget')).not.toBeInTheDocument()
  })

  it('says it is over the budget when it is, and names the reads that break it, and no read that keeps it', async () => {
    open({ metrics: 'slow' })
    const found = await section()
    expect(await found.findByText(/Over the budget: p95 ≤ 250 ms\./)).toBeInTheDocument()
    const broken = within(found.getByText('Reads over the budget').parentElement as HTMLElement)
    expect(broken.getByText('GET /api/orgs/{org}/teams/{team}/view')).toBeInTheDocument()
    expect(broken.queryByText('GET /api/me')).not.toBeInTheDocument()
  })

  it('lists the route groups with their own words, busiest first, each with its routes and which are in-node reads', async () => {
    open({ metrics: 'usual' })
    const found = await section()
    await found.findAllByText('Requests')
    const [serve] = found.getAllByRole('table')
    const groups = within(serve as HTMLElement).getAllByRole('rowheader').filter((one) => one.closest('tr')?.classList.contains('mt-group')).map((one) => one.textContent)
    expect(groups).toEqual(['Teams', 'Desk', 'Usage', 'Evaluation', 'Devices'])
    const teamsView = row(found, 'GET /api/orgs/{org}/teams/{team}/viewIn-node read')
    expect(teamsView).toHaveTextContent('In-node read')
    expect(row(found, 'GET /api/orgs/{org}/teams/{team}/activity')).not.toHaveTextContent('In-node read')
  })

  it('gives p50, p95, and p99 as at most, from the group’s buckets put together, and a p99 beyond the last bound as slower than 30 s', async () => {
    open({ metrics: 'usual' })
    const found = await section()
    await found.findAllByText('Requests')
    const desk = within(row(found, 'Desk'))
    // 4 040 durations, 3 200 of them in the 2 ms bucket and 800 in the 5 ms: p50 is the 2 020th, p95 the 3 838th, p99 the 4 000th.
    expect(desk.getAllByRole('cell').map((cell) => cell.textContent)).toEqual(['4,040', '0', '≤ 2 ms', '≤ 5 ms', '≤ 5 ms'])
    const usage = within(row(found, 'Usage'))
    expect(usage.getAllByRole('cell').map((cell) => cell.textContent)).toEqual(['102', '0', '≤ 1 s', '≤ 5 s', 'slower than 30 s'])
  })

  it('counts the errors of a group from its 5xx series', async () => {
    open({ metrics: 'usual' })
    const found = await section()
    await found.findAllByText('Requests')
    expect(within(row(found, 'Teams')).getAllByRole('cell')[1]?.textContent).toBe('4')
  })

  it('tells the calls the node makes apart from the routes it answers, a peer by its host', async () => {
    open({ metrics: 'usual' })
    const found = await section()
    await found.findAllByText('Requests')
    const calls = within(found.getByRole('heading', { name: 'Calls this node makes' }).closest('.card') as HTMLElement)
    expect(calls.getByRole('rowheader', { name: 'Other nodes' })).toBeInTheDocument()
    expect(calls.getByText('peer get_view (node-b.example)')).toBeInTheDocument()
    expect(calls.getByRole('rowheader', { name: 'Team runtime' })).toBeInTheDocument()
    expect(calls.queryByText('Desk')).not.toBeInTheDocument()
  })

  it('says each objective, what the node says of it, and what it measured, and says "cannot be told" for one it cannot tell', async () => {
    open({ metrics: 'slow' })
    const found = await section()
    const list = within((await found.findByText('Objectives')).closest('.card') as HTMLElement)
    const items = list.getAllByRole('listitem')
    expect(items[0]).toHaveTextContent('Teams: p95 ≤ 100 ms')
    expect(items[0]).toHaveTextContent('Not met')
    expect(items[0]).toHaveTextContent('Measured: ≤ 1 s.')
    expect(items[1]).toHaveTextContent('Desk: p95 ≤ 50 ms')
    expect(items[1]).toHaveTextContent('Met')
    expect(items[2]).toHaveTextContent('Cannot be told yet')
    expect(items[2]).not.toHaveTextContent('Measured')
  })

  it('reads another period from the node and keeps the first until it comes', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'usual' })
    const asked: string[] = []
    const client = createApiClient((request) => {
      const url = new URL(request.url, 'http://mock.local')
      if (url.pathname === '/api/metrics') asked.push(url.searchParams.get('days') ?? '')
      return node.handle(request)
    })
    const user = userEvent.setup()
    renderApp({ path: '/evaluation', node, client })
    const found = await section()
    await found.findAllByText('Requests')
    await user.click(found.getByRole('button', { name: 'Last 7 days' }))
    await waitFor(() => expect(asked).toEqual(['1', '7']))
    expect(found.getByRole('button', { name: 'Last 7 days' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('reads in Japanese, with the words of the glossary', async () => {
    open({ metrics: 'usual' }, { locale: 'ja' })
    const found = within((await screen.findByRole('heading', { level: 2, name: 'このノードの応答時間' })).closest('section') as HTMLElement)
    expect(await found.findByText(/予算の内です: p95 は 50 ミリ秒 以下。/)).toBeInTheDocument()
    expect(found.getByRole('rowheader', { name: 'デスク' })).toBeInTheDocument()
    expect(found.getAllByText('ルートの区分')).toHaveLength(2)
  })
})
