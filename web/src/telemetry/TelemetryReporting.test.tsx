import { QueryClientProvider } from '@tanstack/react-query'
import { render } from '@testing-library/react'
import { createApiClient } from '../api/client'
import { ApiProvider } from '../api/context'
import { createQueryClient } from '../app/queryClient'
import { createMockNode } from '../mock/node'
import { REPORT_EVERY_MS } from './reporter'
import { createTelemetry } from './telemetry'
import { TelemetryReporting } from './TelemetryReporting'

/** The page's reporting over a mock node: the client tells the telemetry how long each call took, as the app wires it. */
function reportingOver(node: ReturnType<typeof createMockNode>, answer: (request: Request) => Promise<Response> = (request) => node.handle(request)) {
  const telemetry = createTelemetry()
  const started = vi.spyOn(telemetry, 'start')
  const api = createApiClient(answer, { hostname: () => 'localhost', isOnline: () => true }, { onSample: telemetry.record })
  const shown = render(
    <QueryClientProvider client={createQueryClient()}>
      <ApiProvider client={api}>
        <TelemetryReporting telemetry={telemetry} />
      </ApiProvider>
    </QueryClientProvider>,
  )
  /** The page has read who the viewer is and, for the owner, has begun to report: before that, a minute passing sends nothing. */
  const isReporting = async (): Promise<void> => vi.waitFor(() => expect(started).toHaveBeenCalled())
  return { api, telemetry, started, isReporting, ...shown }
}

const minute = async (): Promise<void> => {
  await vi.advanceTimersByTimeAsync(REPORT_EVERY_MS)
}

afterEach(() => {
  vi.useRealTimers()
})

describe('reporting how long the page’s calls took, for the owner', () => {
  it('sends what the owner’s page kept after a minute, and the node keeps it as a web client’s, by route template', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    const node = createMockNode()
    const { api, isReporting } = reportingOver(node)
    await isReporting()
    await api.teamSnapshot('acme', 'app')
    expect(node.telemetry()).toEqual([])
    await minute()
    await vi.waitFor(() => expect(node.telemetry().length).toBeGreaterThan(0))
    const view = node.telemetry().find((kept) => kept.route === 'GET /api/orgs/{org}/teams/{team}/view')
    expect(view).toMatchObject({ client: 'web', status: '2xx' })
    expect(JSON.stringify(node.telemetry())).not.toContain('acme')
  })

  it('never sends for a hosted member, and forgets what it kept', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    const node = createMockNode({ guest: { member: 'dave', label: 'Dave' } })
    const calls: string[] = []
    const { api, telemetry, started } = reportingOver(node, (request) => {
      calls.push(`${request.method} ${new URL(request.url).pathname}`)
      return node.handle(request)
    })
    const forbid = vi.spyOn(telemetry, 'forbid')
    await api.me()
    await vi.waitFor(() => expect(forbid).toHaveBeenCalled())
    expect(started).not.toHaveBeenCalled()
    await minute()
    await minute()
    expect(calls.filter((call) => call.endsWith('/api/telemetry'))).toEqual([])
    expect(node.telemetry()).toEqual([])
  })

  it('does not ask again for the page’s life when the node says it does not take this viewer’s reports', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    const node = createMockNode()
    const calls: string[] = []
    const { api, isReporting } = reportingOver(node, (request) => {
      const path = new URL(request.url).pathname
      calls.push(`${request.method} ${path}`)
      return path === '/api/telemetry' ? Promise.resolve(new Response(JSON.stringify({ error: 'owner only' }), { status: 403, headers: { 'content-type': 'application/json' } })) : node.handle(request)
    })
    await isReporting()
    await api.me()
    await minute()
    await vi.waitFor(() => expect(calls.filter((call) => call === 'POST /api/telemetry')).toHaveLength(1))
    await api.me()
    await minute()
    await minute()
    expect(calls.filter((call) => call === 'POST /api/telemetry')).toHaveLength(1)
  })
})
