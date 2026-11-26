import { browserLabel, isMadeWith, isWebPushOffered, keyBytes, subscriptionInput, type BrowserSupport } from './webPush'

const CHROME_MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36'
const support = (overrides: Partial<BrowserSupport> = {}): BrowserSupport => ({ userAgent: CHROME_MAC, touchPoints: 0, isSecureContext: true, hasServiceWorker: true, hasPushManager: true, hasNotification: true, ...overrides })

describe('where Web Push is offered', () => {
  it('is offered in a desktop or Android browser that has it, over a secure context', () => {
    expect(isWebPushOffered(support())).toBe(true)
    expect(isWebPushOffered(support({ userAgent: 'Mozilla/5.0 (Linux; Android 14; Pixel 8) Chrome/130.0 Mobile Safari/537.36' }))).toBe(true)
  })

  it.each([
    ['plain http', { isSecureContext: false }],
    ['no worker', { hasServiceWorker: false }],
    ['no push manager', { hasPushManager: false }],
    ['no notifications', { hasNotification: false }],
    ['an iPhone, which has the app', { userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) Version/18.0 Mobile/15E148 Safari/604.1' }],
    ['an iPad that says it is a Mac, with a touch screen', { touchPoints: 5 }],
  ])('is not offered with %s', (_name, overrides) => {
    expect(isWebPushOffered(support(overrides))).toBe(false)
  })
})

describe('the node’s key', () => {
  it('reads base64url into bytes, and refuses what is not base64url', () => {
    expect([...(keyBytes('BP4_-A') ?? [])]).toEqual([4, 254, 63, 248])
    for (const bad of ['', 'a b', 'a+b/', 'ab=c', '***']) expect(keyBytes(bad), bad).toBeUndefined()
  })

  it('says whether a subscription was made with it', () => {
    const key = Uint8Array.from([4, 1, 2, 3])
    expect(isMadeWith(key.buffer, key)).toBe(true)
    expect(isMadeWith(Uint8Array.from([4, 1, 2, 4]).buffer, key)).toBe(false)
    expect(isMadeWith(Uint8Array.from([4, 1, 2]).buffer, key)).toBe(false)
    expect(isMadeWith(null, key)).toBe(false)
    expect(isMadeWith(undefined, key)).toBe(false)
  })
})

describe('what a subscription is called', () => {
  it.each([
    [CHROME_MAC, 'Chrome on macOS'],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:130.0) Gecko/20100101 Firefox/130.0', 'Firefox on Windows'],
    ['Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0', 'Edge on Windows'],
    ['Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0 Safari/537.36 OPR/115.0', 'Opera on Linux'],
    ['Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15', 'Safari on macOS'],
    ['Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 Chrome/130.0 Safari/537.36', 'Chrome on ChromeOS'],
    ['Mozilla/5.0 (X11; Linux x86_64) Gecko/20100101 Thing/1.0', 'Browser on Linux'],
    ['something else', 'Browser'],
  ])('%s is %s', (userAgent, label) => {
    expect(browserLabel(userAgent)).toBe(label)
  })

  it('names nothing about the person: no version, no machine name', () => {
    expect(browserLabel(CHROME_MAC)).not.toMatch(/\d/)
  })
})

describe('what the node takes of a subscription', () => {
  it('needs the endpoint and both keys', () => {
    expect(subscriptionInput({ endpoint: 'https://push.example/x', keys: { p256dh: 'P', auth: 'A' } }, 'Chrome on macOS')).toEqual({ endpoint: 'https://push.example/x', p256dh: 'P', auth: 'A', label: 'Chrome on macOS' })
    for (const incomplete of [{}, { endpoint: 'https://push.example/x' }, { endpoint: 'https://push.example/x', keys: { p256dh: 'P' } }, { keys: { p256dh: 'P', auth: 'A' } }]) {
      expect(subscriptionInput(incomplete as PushSubscriptionJSON, 'x')).toBeUndefined()
    }
  })
})

describe('the real browser’s stand-in for the worker’s messages', () => {
  it('has nothing to listen to, and does not fail, where the worker container cannot be listened to', async () => {
    const { realBrowser } = await import('./useWebPush')
    const was = Object.getOwnPropertyDescriptor(navigator, 'serviceWorker')
    Object.defineProperty(navigator, 'serviceWorker', { value: { ready: new Promise(() => undefined) }, configurable: true })
    try {
      let heard = 0
      const stop = realBrowser.onSubscriptionChanged(() => void (heard += 1))
      expect(typeof stop).toBe('function')
      expect(() => stop()).not.toThrow()
      expect(heard).toBe(0)
    } finally {
      if (was === undefined) delete (navigator as { serviceWorker?: unknown }).serviceWorker
      else Object.defineProperty(navigator, 'serviceWorker', was)
    }
  })
})
