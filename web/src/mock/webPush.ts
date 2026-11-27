import type { PushSubscription, PushSubscriptionInput, VapidKey } from '../api/clientApi'

/** The application server key of RFC 8291's appendix A: a real point, which a browser accepts as `applicationServerKey`. */
export const MOCK_VAPID_KEY: VapidKey = { publicKey: 'BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8', keyId: 'a1b2c3d4e5f6' }

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

export const idOf = (endpoint: string): string => {
  let hash = 0
  for (const character of endpoint) hash = (hash * 31 + character.charCodeAt(0)) >>> 0
  return hash.toString(16).padStart(8, '0')
}

/** The node's Web Push routes, for a mock node that has them: the key, and one subscription for each endpoint. */
/** `isFailing`: the node's record of every browser says its push service took a notice a day ago and refused the last one an hour ago. */
export function createMockWebPush(now: () => string, isFailing = false, currentDevices?: () => ReadonlySet<string>) {
  let subscriptions: readonly PushSubscription[] = []
  return {
    subscriptions: () => subscriptions,
    /** A removal of a device drops the registrations that name one the account no longer has; those that name none stay. */
    dropWithoutDevice: (current: ReadonlySet<string>) => {
      subscriptions = subscriptions.filter((one) => one.device === undefined || current.has(one.device))
    },
    /** What the node comes to know of a browser's push service: the time it last took a notice, and the last time it did not. */
    recordDelivery: (id: string, delivery: Pick<PushSubscription, 'lastSucceededAt' | 'lastFailure'>) => {
      subscriptions = subscriptions.map((one) => (one.id === id ? { ...one, ...delivery } : one))
    },
    /** Answers a Web Push route, or nothing when the request is not one. `isWriteOk` is the node's check of the write token. */
    handle(request: Request, url: URL, write: () => Promise<Response | { body: unknown }>): Promise<Response> | undefined {
      if (request.method === 'GET' && url.pathname === '/api/push/vapid') return Promise.resolve(json(MOCK_VAPID_KEY))
      if (request.method === 'GET' && url.pathname === '/api/push/subscriptions') return Promise.resolve(json({ subscriptions }))
      if (request.method === 'POST' && url.pathname === '/api/push/subscriptions') {
        return write().then((answer) => {
          if (answer instanceof Response) return answer
          const input = answer.body as PushSubscriptionInput | null
          // A node without `pushDevice` refuses a field it does not know; one with it takes `device` only for a current device of the account.
          if (input !== null && input.device !== undefined && (currentDevices === undefined || !currentDevices().has(input.device))) return json({ error: currentDevices === undefined ? 'unknown field `device`' : 'that is not a device of this account' }, 400)
          if (input === null || typeof input.endpoint !== 'string' || !input.endpoint.startsWith('https://')) return json({ error: 'the endpoint is not one of an allowed push service' }, 400)
          const known = subscriptions.find((one) => one.id === idOf(input.endpoint))
          const refused = isFailing ? { lastSucceededAt: new Date(Date.parse(now()) - 24 * 3_600_000).toISOString(), lastFailure: { at: new Date(Date.parse(now()) - 3_600_000).toISOString(), reason: 'push.example answered 503' } } : {}
          const entry: PushSubscription = { ...refused, ...(known?.lastSucceededAt === undefined ? {} : { lastSucceededAt: known.lastSucceededAt }), ...(known?.lastFailure === undefined ? {} : { lastFailure: known.lastFailure }), id: idOf(input.endpoint), endpoint: `${new URL(input.endpoint).origin}/…${input.endpoint.slice(-8)}`, label: input.label, registeredAt: now(), events: input.events ?? ['request'], keyId: MOCK_VAPID_KEY.keyId, ...(input.device === undefined ? {} : { device: input.device }) }
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
