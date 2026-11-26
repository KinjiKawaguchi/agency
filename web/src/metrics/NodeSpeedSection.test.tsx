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

  it('draws nothing of the section before the node has answered, only a skeleton: no heading, no words, no buttons', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'usual' })
    const { client, release } = heldNode(node, ['/api/metrics'])
    renderApp({ path: '/evaluation', node, client })
    await screen.findByRole('heading', { level: 1, name: 'Evaluation' })
    await waitFor(() => expect(document.querySelector('.skeleton.box')).not.toBeNull())
    expect(screen.queryByText(TITLE)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Last 7 days' })).not.toBeInTheDocument()
    release()
    const found = await section()
    expect(await found.findAllByText('Requests')).toHaveLength(2)
  })

  it('leaves nothing behind when the answer that was waited for is a 403: the heading never appears', async () => {
    const node = createMockNode({ evaluation: true })
    const { client, release } = heldNode(node, ['/api/metrics'])
    renderApp({ path: '/evaluation', node, client })
    await screen.findByRole('heading', { level: 1, name: 'Evaluation' })
    expect(screen.queryByText(TITLE)).not.toBeInTheDocument()
    release()
    await waitFor(() => expect(document.querySelector('.mt-section, .skeleton.box')).toBeNull())
    expect(screen.queryByText(TITLE)).not.toBeInTheDocument()
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('says one line when the node does not measure, and an alert when it fails', async () => {
    open({ metrics: 'absent' })
    expect(await (await section()).findByText('This node does not measure its response time yet.')).toBeInTheDocument()
  })

  it('says what failed when the node cannot give the measurements, and still draws the rest of the page', async () => {
    const { unmount } = open({ metricsFailWith: 503 })
    expect(await (await section()).findByRole('alert')).toHaveTextContent('The node could not give its measurements.')
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
    expect(items[2]).toHaveTextContent('Usage: p95 ≤ 3 s')
    expect(items[2]).toHaveTextContent('Cannot be told against this objective')
    expect(items[2]).toHaveTextContent('Measured: ≤ 5 s.')
    expect(items[2]).not.toHaveTextContent(/\bMet\b|Not met/)
    expect(items[3]).toHaveTextContent('Cannot be told yet')
    expect(items[3]).not.toHaveTextContent('Measured')
  })

  it('does not say an answer it cannot read in the words of its own fields', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'usual' })
    const client = createApiClient(async (request) => {
      const answer = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/metrics') return answer
      const body = (await answer.json()) as { series: Array<Record<string, unknown>> }
      return new Response(JSON.stringify({ ...body, series: body.series.map((one) => ({ ...one, buckets: [1] })) }), { status: 200, headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/evaluation', node, client })
    const alert = await (await section()).findByRole('alert')
    expect(alert).toHaveTextContent('The node could not give its measurements.')
    expect(alert).not.toHaveTextContent(/buckets/)
  })

  it('names a route group by the node’s own word when it has no name of its own, and never by what every object has', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'usual' })
    const client = createApiClient(async (request) => {
      const answer = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/metrics') return answer
      const body = (await answer.json()) as { series: Array<Record<string, unknown>> }
      return new Response(JSON.stringify({ ...body, series: body.series.map((one) => (one.group === 'usage' ? { ...one, group: 'constructor' } : one.group === 'devices' ? { ...one, group: 'brand-new' } : one)) }), { status: 200, headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/evaluation', node, client })
    const found = await section()
    expect(await found.findByRole('rowheader', { name: 'constructor' })).toBeInTheDocument()
    expect(found.getByRole('rowheader', { name: 'brand-new' })).toBeInTheDocument()
  })

  it('says it cannot tell against a budget that falls inside the bucket of the p95, and does not say over', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'slow' })
    const client = createApiClient(async (request) => {
      const answer = await node.handle(request)
      if (new URL(request.url, 'http://mock.local').pathname !== '/api/metrics') return answer
      const body = (await answer.json()) as Record<string, unknown>
      return new Response(JSON.stringify({ ...body, budget: { inNodeReadP95Ms: 175 } }), { status: 200, headers: { 'content-type': 'application/json' } })
    })
    renderApp({ path: '/evaluation', node, client })
    const found = await section()
    expect(await found.findByText(/Cannot be told against the budget: p95 ≤ 250 ms, and the budget of 175 ms lies inside that range\./)).toBeInTheDocument()
    expect(found.queryByText(/Over the budget|Within the budget/)).not.toBeInTheDocument()
  })

  it('draws a skeleton in place of the numbers while another period comes, not the numbers of the first under the second one’s button', async () => {
    const node = createMockNode({ evaluation: true, metrics: 'usual' })
    let release: () => void = () => undefined
    const held = new Promise<void>((resolve) => {
      release = resolve
    })
    const client = createApiClient(async (request) => {
      const url = new URL(request.url, 'http://mock.local')
      if (url.pathname === '/api/metrics' && url.searchParams.get('days') === '7') await held
      return node.handle(request)
    })
    const user = userEvent.setup()
    renderApp({ path: '/evaluation', node, client })
    const found = await section()
    await found.findAllByText('Requests')
    await user.click(found.getByRole('button', { name: 'Last 7 days' }))
    expect(found.getByRole('button', { name: 'Last 7 days' })).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(found.queryByRole('table')).not.toBeInTheDocument())
    expect(document.querySelector('.mt-section .skeleton.box')).not.toBeNull()
    release()
    expect(await found.findAllByText('Requests')).toHaveLength(2)
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
