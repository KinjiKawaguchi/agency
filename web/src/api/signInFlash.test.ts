import { createMockNode } from '../mock/node'
import { createApiClient, type SessionState } from './client'
import { SignInExpiredError } from './session'

const AWAY = { hostname: () => 'agency.example.test', isOnline: () => true }

const isProbe = (request: Request): boolean => new URL(request.url).pathname === '/api/session'

/** What a browser answers for a call that does not follow a redirect, when Access redirects it to its login. */
const redirected = (): Response => new Response(null, { status: 302, headers: { location: 'https://login.example.test/' } })

/** A node whose sign-in is gone: a call that follows the redirect to another origin fails as a network error, and one that does not gets the redirect. */
const signedOut = (request: Request): Promise<Response> => (request.redirect === 'manual' ? Promise.resolve(redirected()) : Promise.reject(new TypeError('Failed to fetch')))

/** What the client told, in order, about the sign-in. */
function watch(client: ReturnType<typeof createApiClient>): SessionState[] {
  const told: SessionState[] = []
  client.onSession((state) => told.push(state))
  return told
}

describe('a notice that the sign-in expired is not shown for a moment', () => {
  it('does not say so for a call that failed once and works again: that was the connection, not the sign-in', async () => {
    const node = createMockNode()
    let fails = 1
    const client = createApiClient((request) => (fails-- > 0 ? Promise.reject(new TypeError('Failed to fetch')) : node.handle(request)), AWAY, { confirmAfterMs: 0 })
    const told = watch(client)
    const failure = await client.me().catch((error: unknown) => error)
    expect(failure).toBeInstanceOf(TypeError)
    expect(failure).not.toBeInstanceOf(SignInExpiredError)
    await client.me()
    expect(told).not.toContain('expired')
  })

  it('does not say so for a call the page itself cancelled, though the sign-in is gone', async () => {
    const controller = new AbortController()
    const client = createApiClient(
      (request) =>
        new Promise((_resolve, reject) => {
          if (request.signal.aborted) return reject(new DOMException('The operation was aborted.', 'AbortError'))
          request.signal.addEventListener('abort', () => reject(new DOMException('The operation was aborted.', 'AbortError')))
          // Anything else gets Access's redirect: a call that asks whether the sign-in holds would say it is gone.
          if (isProbe(request)) _resolve(redirected())
        }),
      AWAY,
      { confirmAfterMs: 0 },
    )
    const told = watch(client)
    const pending = client.activity('acme', 'app', '2026-01-01T00:00:00Z', controller.signal).catch((error: unknown) => error)
    controller.abort()
    expect(await pending).toMatchObject({ name: 'AbortError' })
    expect(told).not.toContain('expired')
  })

  it('does not say so when other calls fail in the same moment because the connection is down, and the node answers the next one', async () => {
    const node = createMockNode()
    let reachable = false
    const client = createApiClient((request) => (reachable ? node.handle(request) : Promise.reject(new TypeError('Failed to fetch'))), AWAY, { confirmAfterMs: 0 })
    const told = watch(client)
    const results = await Promise.allSettled([client.me(), client.decisions(5), client.devices()])
    expect(results.map((one) => one.status)).toEqual(['rejected', 'rejected', 'rejected'])
    expect(results.every((one) => one.status === 'rejected' && !(one.reason instanceof SignInExpiredError))).toBe(true)
    reachable = true
    await client.me()
    expect(told).not.toContain('expired')
  })

  it('says so when the node, asked a moment later without following a redirect, answers with Access’s redirect, and then clears it when a call works', async () => {
    const node = createMockNode()
    let isExpired = true
    const client = createApiClient((request) => (isExpired ? signedOut(request) : node.handle(request)), AWAY, { confirmAfterMs: 0 })
    const told = watch(client)
    await expect(client.me()).rejects.toBeInstanceOf(SignInExpiredError)
    expect(told).toEqual(['expired'])
    isExpired = false
    await client.me()
    expect(told).toEqual(['expired', 'ok'])
  })

  it('does not say so when the connection is down for longer than the wait: only a redirect is a lost sign-in', async () => {
    const client = createApiClient(() => Promise.reject(new TypeError('Failed to fetch')), AWAY, { confirmAfterMs: 0 })
    const told = watch(client)
    const failure = await client.me().catch((error: unknown) => error)
    expect(failure).toBeInstanceOf(TypeError)
    expect(failure).not.toBeInstanceOf(SignInExpiredError)
    expect(told).not.toContain('expired')
  })

  it.each([200, 401, 500, 503])('does not say so when the node answers the question at all (%i): the sign-in holds', async (status) => {
    const client = createApiClient((request) => (isProbe(request) ? Promise.resolve(new Response('{}', { status })) : Promise.reject(new TypeError('Failed to fetch'))), AWAY, { confirmAfterMs: 0 })
    const told = watch(client)
    await expect(client.me()).rejects.toBeInstanceOf(TypeError)
    expect(told).not.toContain('expired')
  })

  it('asks without following a redirect, which is how Access’s redirect is told from a lost connection', async () => {
    const seen: string[] = []
    const client = createApiClient(
      (request) => {
        if (isProbe(request)) seen.push(request.redirect)
        return signedOut(request)
      },
      AWAY,
      { confirmAfterMs: 0 },
    )
    await client.me().catch(() => undefined)
    expect(seen).toEqual(['manual'])
  })

  it('takes the answer a browser gives for a redirect it did not follow, an opaque one, for the redirect', async () => {
    const opaque = Object.defineProperty(new Response(null), 'type', { value: 'opaqueredirect' })
    const client = createApiClient((request) => (isProbe(request) ? Promise.resolve(opaque) : Promise.reject(new TypeError('Failed to fetch'))), AWAY, { confirmAfterMs: 0 })
    await expect(client.me()).rejects.toBeInstanceOf(SignInExpiredError)
  })

  it('asks the node once, however many calls failed at the same time', async () => {
    let sessionCalls = 0
    const client = createApiClient(
      (request) => {
        if (isProbe(request)) sessionCalls += 1
        return signedOut(request)
      },
      AWAY,
      { confirmAfterMs: 0 },
    )
    await Promise.allSettled([client.me(), client.decisions(5), client.devices(), client.me()])
    expect(sessionCalls).toBe(1)
  })

  it('does not say so for a call that failed while online if the browser went offline while it waited', async () => {
    vi.useFakeTimers()
    try {
      let isOnline = true
      const client = createApiClient(signedOut, { hostname: AWAY.hostname, isOnline: () => isOnline }, { confirmAfterMs: 1000 })
      const told = watch(client)
      const call = client.me().catch((error: unknown) => error)
      await vi.advanceTimersByTimeAsync(100)
      isOnline = false
      await vi.advanceTimersByTimeAsync(1000)
      expect(await call).toBeInstanceOf(TypeError)
      expect(told).not.toContain('expired')
    } finally {
      vi.useRealTimers()
    }
  })

  it('does not say so for a call that failed when the browser was already offline', async () => {
    const client = createApiClient(() => Promise.reject(new TypeError('Failed to fetch')), { hostname: AWAY.hostname, isOnline: () => false }, { confirmAfterMs: 0 })
    const told = watch(client)
    await expect(client.me()).rejects.toBeInstanceOf(TypeError)
    expect(told).not.toContain('expired')
  })

  it('waits before it asks, so that a connection that comes back at once is not taken for a lost sign-in', async () => {
    vi.useFakeTimers()
    try {
      const node = createMockNode()
      let isUp = false
      const client = createApiClient((request) => (isUp ? node.handle(request) : Promise.reject(new TypeError('Failed to fetch'))), AWAY, { confirmAfterMs: 1000 })
      const told = watch(client)
      const call = client.me().catch((error: unknown) => error)
      await vi.advanceTimersByTimeAsync(500)
      isUp = true
      await vi.advanceTimersByTimeAsync(600)
      expect(await call).toBeInstanceOf(TypeError)
      expect(told).not.toContain('expired')
    } finally {
      vi.useRealTimers()
    }
  })
})
