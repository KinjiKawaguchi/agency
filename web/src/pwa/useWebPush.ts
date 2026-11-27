import { useCallback, useEffect, useRef, useState } from 'react'
import { ApiError } from '../api/client'
import type { ApiClient } from '../api/client'
import { type BrowserSupport, type PushDelivery, browserLabel, deliveryOf, isMadeWith, isWebPushOffered, keyBytes, subscriptionInput } from './webPush'

/**
 * Where Web Push stands in this browser, for the Devices screen. `unsupported` (this browser has none, or it is an
 * iPhone, which has the app), `unavailable` (the node has no `webPush`), `denied` (the viewer refused notifications for
 * this page), `off`, `stale` (subscribed with a key the node no longer has: subscribe again), `on` (with what the node knows of how its push
 * service took the notices sent so far).
 */
export type WebPushState = { kind: 'loading' } | { kind: 'unsupported' } | { kind: 'unavailable' } | { kind: 'denied' } | { kind: 'off' } | { kind: 'stale' } | { kind: 'on'; delivery: PushDelivery } | { kind: 'failed'; message: string }

/** What the hook needs of the browser, so that a test stands in for it. */
export interface WebPushBrowser {
  support: () => BrowserSupport
  permission: () => NotificationPermission
  requestPermission: () => Promise<NotificationPermission>
  /** The worker's registration; waits until the worker is ready. */
  registration: () => Promise<{ pushManager: PushManager }>
  /** Calls `listener` when the worker says the browser renewed the subscription; returns the way to stop. */
  onSubscriptionChanged: (listener: () => void) => () => void
}

export const realBrowser: WebPushBrowser = {
  support: () => ({
    userAgent: navigator.userAgent,
    touchPoints: navigator.maxTouchPoints,
    isSecureContext: window.isSecureContext,
    hasServiceWorker: 'serviceWorker' in navigator,
    hasPushManager: 'PushManager' in window,
    hasNotification: 'Notification' in window,
  }),
  permission: () => Notification.permission,
  requestPermission: () => Notification.requestPermission(),
  registration: () => navigator.serviceWorker.ready,
  onSubscriptionChanged: (listener) => {
    // A page that has a worker container which cannot be listened to (a stand-in, an old browser) has nothing to hear.
    if (typeof navigator.serviceWorker.addEventListener !== 'function') return () => undefined
    const onMessage = (event: MessageEvent): void => {
      if (typeof event.data === 'object' && event.data !== null && 'type' in event.data && event.data.type === 'push-subscription-changed') listener()
    }
    navigator.serviceWorker.addEventListener('message', onMessage)
    return () => navigator.serviceWorker.removeEventListener('message', onMessage)
  },
}

type Api = Pick<ApiClient, 'about' | 'vapidKey' | 'registerPushSubscription' | 'removePushSubscription'>

/** The capability of a node that ties a registration to the device it names, and drops it when that device is removed. A node without it refuses a field it does not know. */
const PUSH_DEVICE = 'pushDevice'

/** The device a registration names: this browser's, when the node knows what to do with it (`pushDevice`) and this browser is added. */
async function deviceToName(api: Api, device: string | undefined): Promise<string | undefined> {
  if (device === undefined) return undefined
  const about = await api.about().catch(() => undefined)
  return about?.capabilities.includes(PUSH_DEVICE) === true ? device : undefined
}

const failed = (error: unknown): WebPushState => ({ kind: 'failed', message: error instanceof Error ? error.message : String(error) })

/** Registers what the browser holds at the node (idempotent), and says whether it can be sent to. */
async function stateWithSubscription(api: Api, browser: WebPushBrowser, subscription: PushSubscription, key: Uint8Array, device: string | undefined): Promise<WebPushState> {
  if (!isMadeWith(subscription.options.applicationServerKey, key)) return { kind: 'stale' }
  const input = subscriptionInput(subscription.toJSON(), browserLabel(browser.support().userAgent), await deviceToName(api, device))
  if (input === undefined) return { kind: 'stale' }
  return { kind: 'on', delivery: deliveryOf(await api.registerPushSubscription(input)) }
}

async function read(api: Api, browser: WebPushBrowser, device: string | undefined): Promise<WebPushState> {
  if (!isWebPushOffered(browser.support())) return { kind: 'unsupported' }
  let key: Uint8Array | undefined
  try {
    key = keyBytes((await api.vapidKey()).publicKey)
  } catch (error) {
    return error instanceof ApiError && error.status === 404 ? { kind: 'unavailable' } : failed(error)
  }
  if (key === undefined) return failed(new Error('The node sent a key that is not one.'))
  if (browser.permission() === 'denied') return { kind: 'denied' }
  const subscription = await (await browser.registration()).pushManager.getSubscription()
  if (subscription === null || browser.permission() !== 'granted') return { kind: 'off' }
  return stateWithSubscription(api, browser, subscription, key, device)
}

async function subscribe(api: Api, browser: WebPushBrowser, device: string | undefined): Promise<WebPushState> {
  // The permission is asked first and at once, inside the click: Firefox and Safari take a request that comes after an
  // await (the node's key is read) as not made by the person, and refuse it.
  const permission = browser.permission() === 'granted' ? 'granted' : await browser.requestPermission()
  if (permission !== 'granted') return permission === 'denied' ? { kind: 'denied' } : { kind: 'off' }
  const key = keyBytes((await api.vapidKey()).publicKey)
  if (key === undefined) return failed(new Error('The node sent a key that is not one.'))
  const { pushManager } = await browser.registration()
  const earlier = await pushManager.getSubscription()
  if (earlier !== null && !isMadeWith(earlier.options.applicationServerKey, key)) await earlier.unsubscribe()
  const subscription = await pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key })
  return stateWithSubscription(api, browser, subscription, key, device)
}

/** Takes this browser's subscription off the node and then off the browser. The node's id is what registering the same endpoint answers. */
async function unsubscribe(api: Api, browser: WebPushBrowser): Promise<WebPushState> {
  const subscription = await (await browser.registration()).pushManager.getSubscription()
  if (subscription === null) return { kind: 'off' }
  const input = subscriptionInput(subscription.toJSON(), browserLabel(browser.support().userAgent))
  if (input !== undefined) await api.removePushSubscription((await api.registerPushSubscription(input)).id)
  await subscription.unsubscribe()
  return { kind: 'off' }
}

/**
 * Registers at the node the subscription this browser holds, when it holds one made with the node's key and still
 * has the permission. It asks for nothing, and reads nothing from the node unless there is something to register.
 * The page does it wherever the viewer is (not only on the Devices screen), so that a subscription the browser
 * renewed while the app was closed is registered the next time any page of the app is opened.
 */
export async function registerHeldSubscription(api: Api, browser: WebPushBrowser, device?: string): Promise<void> {
  if (!isWebPushOffered(browser.support()) || browser.permission() !== 'granted') return
  const subscription = await (await browser.registration()).pushManager.getSubscription()
  if (subscription === null) return
  const key = await api.vapidKey().then(
    (found) => keyBytes(found.publicKey),
    () => undefined,
  )
  if (key !== undefined) await stateWithSubscription(api, browser, subscription, key, device)
}

export function useWebPush(api: Api, browser: WebPushBrowser = realBrowser, device?: string): { state: WebPushState; turnOn: () => void; turnOff: () => void } {
  const [state, setState] = useState<WebPushState>({ kind: 'loading' })
  // The device a registration names is read when the registration is made. Its arrival does not read the node again here: the page's own
  // registration (`WebPushRegistration`) tells the node when this browser is added.
  const deviceNow = useRef(device)
  deviceNow.current = device
  const run = useCallback(
    (work: () => Promise<WebPushState>) => {
      work().then(setState, (error: unknown) => setState(failed(error)))
    },
    [],
  )
  useEffect(() => {
    run(() => read(api, browser, deviceNow.current))
    return browser.support().hasServiceWorker ? browser.onSubscriptionChanged(() => run(() => read(api, browser, deviceNow.current))) : undefined
  }, [api, browser, run])
  return {
    state,
    turnOn: () => run(() => subscribe(api, browser, deviceNow.current)),
    turnOff: () => run(() => unsubscribe(api, browser)),
  }
}
