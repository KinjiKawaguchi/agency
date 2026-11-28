import { ApiError, createApiClient } from '../api/client'
import { SignInExpiredError } from '../api/session'
import type { Sample } from './samples'

const json = (status: number, body: unknown = {}): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

/** A client whose node answers with `answer` after `takes` milliseconds of a clock that the test moves, and the samples it told. */
function measuring(answer: (request: Request) => Response | Promise<Response>, takes = 0) {
  const samples: Sample[] = []
  let now = 1_000
  const api = createApiClient(
    async (request) => {
      now += takes
      return answer(request)
    },
    { hostname: () => 'localhost', isOnline: () => true },
    { onSample: (sample) => samples.push(sample), clock: () => now, confirmAfterMs: 0 },
  )
  return { api, samples }
}

describe('how long the page’s calls took, as the client tells it', () => {
  it('is the route template, "2xx", and the whole milliseconds until the body was read, never the path', async () => {
    const { api, samples } = measuring(() => json(200, { teams: [] }), 12.6)
    await api.teamSnapshot('acme', 'app').catch(() => undefined)
    expect(samples).toEqual([{ route: 'GET /api/orgs/{org}/teams/{team}/view', status: '2xx', ms: 13 }])
    expect(JSON.stringify(samples)).not.toContain('acme')
  })

  it('is the class of the answer when the node refuses or fails: 4xx and 5xx, as the calls that were refused took', async () => {
    const refused = measuring(() => json(403, { error: 'only the owner' }), 5)
    await expect(refused.api.outcomes('acme', 30)).rejects.toBeInstanceOf(ApiError)
    expect(refused.samples).toEqual([{ route: 'GET /api/orgs/{org}/outcomes', status: '4xx', ms: 5 }])
    const failing = measuring(() => json(503, { error: 'busy' }), 40)
    await expect(failing.api.me()).rejects.toBeInstanceOf(ApiError)
    expect(failing.samples).toEqual([{ route: 'GET /api/me', status: '5xx', ms: 40 }])
  })

  it('is "network" for a call that got no answer, with the time it took to fail', async () => {
    const { api, samples } = measuring(() => Promise.reject(new TypeError('Failed to fetch')), 7)
    await expect(api.me()).rejects.toBeInstanceOf(TypeError)
    expect(samples).toEqual([{ route: 'GET /api/me', status: 'network', ms: 7 }])
  })

  it('is nothing for a call the page cancelled: it says nothing of the node', async () => {
    const controller = new AbortController()
    const { api, samples } = measuring(() => {
      controller.abort()
      return Promise.reject(new DOMException('aborted', 'AbortError'))
    })
    await expect(api.activity('acme', 'app', '2026-11-24T10:00:00Z', controller.signal)).rejects.toBeDefined()
    expect(samples).toEqual([])
  })

  it('is nothing for a call that failed because the sign-in is gone', async () => {
    const samples: Sample[] = []
    const api = createApiClient(
      (request) => (new URL(request.url).pathname === '/api/session' ? Promise.resolve({ type: 'opaqueredirect', status: 0 } as Response) : Promise.reject(new TypeError('Failed to fetch'))),
      { hostname: () => 'agency.example.com', isOnline: () => true },
      { onSample: (sample) => samples.push(sample), confirmAfterMs: 0 },
    )
    await expect(api.me()).rejects.toBeInstanceOf(SignInExpiredError)
    expect(samples.filter((one) => one.route === 'GET /api/me')).toEqual([])
  })

  it('is told for each write, by its template, and a retry after a stale token is a call of its own', async () => {
    let writes = 0
    const { api, samples } = measuring((request) => {
      const path = new URL(request.url).pathname
      if (path === '/api/session') return json(200, { writeToken: 'fresh', now: '2026-11-24T10:00:00Z' })
      writes += 1
      return writes === 1 ? json(403, { error: 'missing or wrong write token' }) : json(200, {})
    })
    await api.registerPushToken({ token: 'ab', environment: 'sandbox', label: 'x' })
    expect(samples.map((one) => `${one.route} ${one.status}`)).toEqual(['POST /api/push/tokens 4xx', 'GET /api/session 2xx', 'POST /api/push/tokens 2xx'])
  })

  it('does not time the call that carries the report, and sends it as a keepalive with the token', async () => {
    let seen: Request | undefined
    const { api, samples } = measuring((request) => {
      seen = request
      return json(200, { accepted: 2, dropped: 1 })
    })
    const told = await api.reportTelemetry([{ route: 'GET /api/me', status: '2xx', ms: 3 }])
    expect(told).toEqual({ accepted: 2, dropped: 1 })
    expect(samples).toEqual([])
    expect(seen?.method).toBe('POST')
    expect(new URL(seen?.url ?? '').pathname).toBe('/api/telemetry')
    expect(seen?.keepalive).toBe(true)
    expect(seen?.headers.get('x-agency-token')).not.toBeNull()
    expect(await seen?.json()).toEqual({ client: 'web', samples: [{ route: 'GET /api/me', status: '2xx', ms: 3 }] })
  })

  it('reads what the node took only when they are numbers, and says nothing it was not told', async () => {
    expect(await measuring(() => json(200, { accepted: '2', dropped: -1 })).api.reportTelemetry([])).toEqual({})
    expect(await measuring(() => json(200, {})).api.reportTelemetry([])).toEqual({})
  })

  it('tells nothing without a place to tell it: a client made with no listener works as before', async () => {
    const api = createApiClient(() => Promise.resolve(json(200, {})))
    await expect(api.me()).resolves.toBeDefined()
  })
})
