import { QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { createApiClient } from '../api/client'
import { ApiProvider } from '../api/context'
import type { BudgetStatus } from '../api/clientApi'
import { createQueryClient } from '../app/queryClient'
import { I18nProvider } from '../i18n'
import { createMockNode } from '../mock/node'
import { TeamBudget } from './TeamBudget'

/** A team's budget over a mock node, and the paths the page asked of it. */
function over(node: ReturnType<typeof createMockNode>, answer: (request: Request) => Promise<Response> = (request) => node.handle(request), planned: number | undefined = 100, team = 'app') {
  const asked: string[] = []
  const api = createApiClient(
    (request) => {
      asked.push(new URL(request.url).pathname)
      return answer(request)
    },
    { hostname: () => 'localhost', isOnline: () => true },
  )
  render(
    <I18nProvider initial="en">
      <QueryClientProvider client={createQueryClient()}>
        <ApiProvider client={api}>
          <TeamBudget org="acme" team={team} planned={planned} />
        </ApiProvider>
      </QueryClientProvider>
    </I18nProvider>,
  )
  return asked
}

const now = new Date('2026-11-25T12:00:00Z')
const refused = (status: number) => (): Promise<Response> => Promise.resolve(new Response(JSON.stringify({ error: 'no' }), { status, headers: { 'content-type': 'application/json' } }))

describe('a team’s weekly budget on the page', () => {
  it('shows the meter of what a node with the capability answers, for the week from Monday', async () => {
    over(createMockNode({ budget: true, now }))
    expect(await screen.findByText('$86 of $100 this week, from Monday 2026-11-23')).toBeInTheDocument()
    expect(screen.getByText('Close to the budget')).toBeInTheDocument()
  })

  it('shows what the node answers for the team, whatever else it knows: the figures of an answer given by team', async () => {
    const answer: BudgetStatus = { weekStart: '2026-11-23', team: { capUsd: 100, spentUsd: 12, state: 'ok', partial: true }, org: { capUsd: 100, spentUsd: 12, state: 'ok', partial: true } }
    over(createMockNode({ budget: { app: answer }, now }))
    expect(await screen.findByText('Not known this week, from Monday 2026-11-23')).toBeInTheDocument()
  })

  it('does not ask a node without the capability, and says the plan’s budget and that the node does not report the spend', async () => {
    const asked = over(createMockNode({ now }))
    expect(await screen.findByText('Weekly budget $100. This node does not report the week’s spend yet.')).toBeInTheDocument()
    expect(asked.filter((path) => path.endsWith('/budget'))).toEqual([])
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('says nothing for a team with no plan on a node without the capability', async () => {
    const asked = over(createMockNode({ now }), undefined, undefined)
    await vi.waitFor(() => expect(asked.some((path) => path === '/api/about')).toBe(true))
    expect(screen.queryByText(/Weekly budget/)).not.toBeInTheDocument()
  })

  it('draws no meter for a team the node says has no cap', async () => {
    const asked = over(createMockNode({ budget: true, now }), undefined, undefined, 'site')
    await vi.waitFor(() => expect(asked.some((path) => path.endsWith('/teams/site/budget'))).toBe(true))
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('does not call a 404 from a node that lists the capability a failure: it says what the plan states', async () => {
    const node = createMockNode({ budget: {}, now })
    over(node)
    expect(await screen.findByText('Weekly budget $100. This node does not report the week’s spend yet.')).toBeInTheDocument()
  })

  it('says a failure in place, with the node’s words, and does not make up a meter', async () => {
    const node = createMockNode({ budget: true, now })
    over(node, (request) => (new URL(request.url).pathname.endsWith('/budget') ? refused(403)() : node.handle(request)))
    expect(await screen.findByRole('alert')).toBeInTheDocument()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
  })

  it('shows a skeleton, not a figure, until the node has answered', async () => {
    const node = createMockNode({ budget: true, now })
    over(node, (request) => (new URL(request.url).pathname.endsWith('/budget') ? new Promise<Response>(() => undefined) : node.handle(request)))
    await vi.waitFor(() => expect(document.querySelector('.skeleton, [aria-busy="true"], [data-skeleton]')).not.toBeNull())
    expect(screen.queryByText(/\$/)).not.toBeInTheDocument()
  })
})
