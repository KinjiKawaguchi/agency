// What the iPhone app hands the page (docs/apple.md, "Hand-over to the page"): the app sets
// `window.agencyNative` before the page runs, and sets it again and dispatches `agency:native-push`
// when the push state changes while the page is open. Outside the app there is no `agencyNative`.
import type { PushTokenInput } from '../api/clientApi'

export const NATIVE_PUSH_EVENT = 'agency:native-push'

/** The APNs device token of this phone, with the environment Apple issued it for and the device model. */
export type NativePush = PushTokenInput

/** The node takes a hex token of 32 to 200 characters, and a label of 1 to 80 (docs/protocol.md, Client API). */
const LOWERCASE_HEX = /^[0-9a-f]{32,200}$/
const MAX_LABEL = 80

/** The push state in the shape the app promises, or undefined when it is missing or malformed. */
export function readPush(value: unknown): NativePush | undefined {
  if (typeof value !== 'object' || value === null) return undefined
  const { token, environment, label } = value as Record<string, unknown>
  if (typeof token !== 'string' || !LOWERCASE_HEX.test(token)) return undefined
  if (environment !== 'sandbox' && environment !== 'production') return undefined
  if (typeof label !== 'string' || label.trim() === '' || label.length > MAX_LABEL) return undefined
  return { token, environment, label: label.trim() }
}

/** The push state the app set on the page, if the page runs in the app and the owner has allowed notifications. */
export function pushOfPage(page: object): NativePush | undefined {
  const native = (page as { agencyNative?: unknown }).agencyNative
  return typeof native === 'object' && native !== null && 'push' in native ? readPush(native.push) : undefined
}
