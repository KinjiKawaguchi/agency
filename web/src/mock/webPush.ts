import type { PushSubscription, PushSubscriptionInput, VapidKey } from '../api/clientApi'

/** The application server key of RFC 8291's appendix A: a real point, which a browser accepts as `applicationServerKey`. */
export const MOCK_VAPID_KEY: VapidKey = { publicKey: 'BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8', keyId: 'a1b2c3d4e5f6' }

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

const idOf = (endpoint: string): string => {
  let hash = 0
  for (const character of endpoint) hash = (hash * 31 + character.charCodeAt(0)) >>> 0
  return hash.toString(16).padStart(8, '0')
}

/** The node's Web Push routes, for a mock node that has them: the key, and one subscription for each endpoint. */
export function createMockWebPush(now: () => string) {
  let subscriptions: readonly PushSubscription[] = []
  return {
    subscriptions: () => subscriptions,
    /** Answers a Web Push route, or nothing when the request is not one. `isWriteOk` is the node's check of the write token. */
    handle(request: Request, url: URL, write: () => Promise<Response | { body: unknown }>): Promise<Response> | undefined {
      if (request.method === 'GET' && url.pathname === '/api/push/vapid') return Promise.resolve(json(MOCK_VAPID_KEY))
      if (request.method === 'GET' && url.pathname === '/api/push/subscriptions') return Promise.resolve(json({ subscriptions }))
      if (request.method === 'POST' && url.pathname === '/api/push/subscriptions') {
        return write().then((answer) => {
          if (answer instanceof Response) return answer
          const input = answer.body as PushSubscriptionInput | null
          if (input === null || typeof input.endpoint !== 'string' || !input.endpoint.startsWith('https://')) return json({ error: 'the endpoint is not one of an allowed push service' }, 400)
          const entry: PushSubscription = { id: idOf(input.endpoint), endpoint: `${new URL(input.endpoint).origin}/…${input.endpoint.slice(-8)}`, label: input.label, registeredAt: now(), events: input.events ?? ['request'], keyId: MOCK_VAPID_KEY.keyId }
          subscriptions = [...subscriptions.filter((one) => one.id !== entry.id), entry]
          return json(entry)
        })
      }
      const removal = /^\/api\/push\/subscriptions\/([^/]+)$/.exec(url.pathname)
      if (request.method === 'DELETE' && removal !== null) {
        return write().then((answer) => {
          if (answer instanceof Response) return answer
          subscriptions = subscriptions.filter((one) => one.id !== decodeURIComponent(removal[1] ?? ''))
          return new Response(null, { status: 204 })
        })
      }
      return undefined
    },
  }
}
