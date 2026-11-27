import type { PushFailure, PushSubscription as NodeSubscription, PushSubscriptionInput } from '../api/clientApi'

/** What the browser has for Web Push. A page opened over plain http, with no worker, or on an iPhone or iPad has none that the app offers. */
export interface BrowserSupport {
  userAgent: string
  /** `navigator.maxTouchPoints`: an iPad asks for the desktop site and says it is a Mac, but has a touch screen. */
  touchPoints: number
  isSecureContext: boolean
  hasServiceWorker: boolean
  hasPushManager: boolean
  hasNotification: boolean
}

const isIos = (support: BrowserSupport): boolean => /iPhone|iPad|iPod/.test(support.userAgent) || (/Macintosh/.test(support.userAgent) && support.touchPoints > 1)

/**
 * Whether the app offers Web Push in this browser: on desktop and Android browsers that have it. The iPhone and the iPad
 * have their own app, which the node notifies through APNs, so the page offers nothing there.
 */
export const isWebPushOffered = (support: BrowserSupport): boolean => support.isSecureContext && support.hasServiceWorker && support.hasPushManager && support.hasNotification && !isIos(support)

/** The bytes of a base64url text, as `applicationServerKey` takes them. A text that is not base64url gives none. */
export function keyBytes(text: string): Uint8Array<ArrayBuffer> | undefined {
  if (!/^[A-Za-z0-9_-]+$/.test(text)) return undefined
  const padded = text.replace(/-/g, '+').replace(/_/g, '/').padEnd(Math.ceil(text.length / 4) * 4, '=')
  try {
    return Uint8Array.from(atob(padded), (char) => char.charCodeAt(0))
  } catch {
    return undefined
  }
}

/** Whether a subscription was made with this key (`applicationServerKey` of its options, as bytes). */
export function isMadeWith(options: ArrayBuffer | null | undefined, key: Uint8Array): boolean {
  if (options === null || options === undefined) return false
  const made = new Uint8Array(options)
  return made.length === key.length && made.every((byte, index) => byte === key[index])
}

const BROWSERS: ReadonlyArray<readonly [RegExp, string]> = [
  [/Edg\//, 'Edge'],
  [/OPR\//, 'Opera'],
  [/Firefox\//, 'Firefox'],
  [/Chrome\//, 'Chrome'],
  [/Safari\//, 'Safari'],
]

const SYSTEMS: ReadonlyArray<readonly [RegExp, string]> = [
  [/Android/, 'Android'],
  [/Windows/, 'Windows'],
  [/CrOS/, 'ChromeOS'],
  [/Macintosh|Mac OS X/, 'macOS'],
  [/Linux|X11/, 'Linux'],
]

/** The label a subscription gets, which the Devices screen shows: the browser and the system, as "Firefox on Windows". Nothing that names a person. */
export function browserLabel(userAgent: string): string {
  const browser = BROWSERS.find(([pattern]) => pattern.test(userAgent))?.[1]
  const system = SYSTEMS.find(([pattern]) => pattern.test(userAgent))?.[1]
  if (browser === undefined) return system === undefined ? 'Browser' : `Browser on ${system}`
  return system === undefined ? browser : `${browser} on ${system}`
}

/** What the node takes, from the browser's subscription; nothing when it lacks what a message is encrypted to. */
// `events` is what this browser asks the node to tell it: a request that waits, and a change that waits for the root key's
// signature (a recovery is told to everyone). Without it the node tells a request only, and the owner would not hear of a held change.
export function subscriptionInput(subscription: PushSubscriptionJSON, label: string, device?: string): PushSubscriptionInput | undefined {
  const { endpoint, keys } = subscription
  if (endpoint === undefined || keys?.p256dh === undefined || keys.auth === undefined) return undefined
  return { endpoint, p256dh: keys.p256dh, auth: keys.auth, label, events: ['request', 'root'], ...(device === undefined ? {} : { device }) }
}

/**
 * What the node knows of how this browser's push service took its notices: `unknown` until it has taken or refused one,
 * `taken` (the time of the last it took), or `failing` (its last refusal is later than its last taking, or it has
 * taken none). The last taking is given when there was one, so that "never" is a missing value and not a zero.
 */
export type PushDelivery = { kind: 'unknown' } | { kind: 'taken'; at: string } | { kind: 'failing'; failure: PushFailure; lastTakenAt?: string }

export function deliveryOf(subscription: Pick<NodeSubscription, 'lastSucceededAt' | 'lastFailure'>): PushDelivery {
  const { lastSucceededAt, lastFailure } = subscription
  if (lastFailure === undefined) return lastSucceededAt === undefined ? { kind: 'unknown' } : { kind: 'taken', at: lastSucceededAt }
  if (lastSucceededAt === undefined) return { kind: 'failing', failure: lastFailure }
  // A time that cannot be read is not a later time: the failure is then what stands, since it is the one that tells something is wrong.
  const isFailureLater = !(Date.parse(lastSucceededAt) >= Date.parse(lastFailure.at))
  return isFailureLater ? { kind: 'failing', failure: lastFailure, lastTakenAt: lastSucceededAt } : { kind: 'taken', at: lastSucceededAt }
}
