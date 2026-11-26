import type { WebPushBrowser } from '../pwa/useWebPush'
import { keyBytes } from '../pwa/webPush'
import { MOCK_VAPID_KEY } from '../mock/webPush'

/** A subscription as the browser holds it: its endpoint, the key it was made with, and what it can say of itself. */
export interface FakeSubscription {
  endpoint: string
  options: { applicationServerKey: ArrayBuffer | null }
  toJSON: () => { endpoint: string; keys: { p256dh: string; auth: string } }
  unsubscribe: () => Promise<boolean>
}

export interface FakePush {
  browser: WebPushBrowser
  /** What the browser holds now: none, or the subscription of this page. */
  subscription: () => FakeSubscription | null
  subscribed: () => readonly unknown[]
  unsubscribed: () => number
  askedPermission: () => number
  /** What the person answers when the browser asks for the permission. */
  answer: (permission: NotificationPermission) => void
  /** The next subscribe fails with this message. */
  failNextSubscribe: (message: string) => void
  /** Starts with a subscription made with this key (the node's by default). */
  holds: (key?: Uint8Array) => void
  /** The page's worker says the browser renewed the subscription. */
  renewed: () => void
}

const USER_AGENTS = { desktop: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36', iphone: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1' }

/** A browser with Web Push, as far as the page sees it. `isOffered: false` is one with none (an iPhone, or a page without a worker). */
export function fakePushBrowser({ permission = 'default', isOffered = true, userAgent = 'desktop' }: { permission?: NotificationPermission; isOffered?: boolean; userAgent?: keyof typeof USER_AGENTS } = {}): FakePush {
  let current: FakeSubscription | null = null
  let granted = permission
  let answer: NotificationPermission = 'granted'
  let nextFailure: string | undefined
  let asked = 0
  let removed = 0
  let madeCount = 0
  const subscribed: unknown[] = []
  const listeners = new Set<() => void>()
  const made = (key: Uint8Array | null): FakeSubscription => {
    madeCount += 1
    const endpoint = `https://fcm.googleapis.com/fcm/send/${madeCount}:fake-endpoint`
    const self: FakeSubscription = {
      endpoint,
      options: { applicationServerKey: key === null ? null : (key.buffer.slice(key.byteOffset, key.byteOffset + key.byteLength) as ArrayBuffer) },
      toJSON: () => ({ endpoint, keys: { p256dh: MOCK_VAPID_KEY.publicKey, auth: 'BTBZMqHH6r4Tts7J_aSIgg' } }),
      unsubscribe: () => ((removed += 1), (current = null), Promise.resolve(true)),
    }
    return self
  }
  const browser: WebPushBrowser = {
    support: () => ({ userAgent: USER_AGENTS[userAgent], touchPoints: 0, isSecureContext: true, hasServiceWorker: isOffered, hasPushManager: isOffered, hasNotification: isOffered }),
    permission: () => granted,
    requestPermission: () => ((asked += 1), (granted = answer), Promise.resolve(answer)),
    registration: () =>
      Promise.resolve({
        pushManager: {
          getSubscription: () => Promise.resolve(current),
          subscribe: (options?: PushSubscriptionOptionsInit) => {
            subscribed.push(options)
            if (nextFailure !== undefined) {
              const message = nextFailure
              nextFailure = undefined
              return Promise.reject(new Error(message))
            }
            current = made(options?.applicationServerKey instanceof Uint8Array ? options.applicationServerKey : null)
            return Promise.resolve(current as unknown as PushSubscription)
          },
        } as unknown as PushManager,
      }),
    onSubscriptionChanged: (listener) => (listeners.add(listener), () => void listeners.delete(listener)),
  }
  return {
    browser,
    subscription: () => current,
    subscribed: () => subscribed,
    unsubscribed: () => removed,
    askedPermission: () => asked,
    answer: (value) => void (answer = value),
    failNextSubscribe: (message) => void (nextFailure = message),
    holds: (key = keyBytes(MOCK_VAPID_KEY.publicKey)) => void (current = made(key ?? null)),
    renewed: () => listeners.forEach((listener) => listener()),
  }
}
