// The number on the iPhone app's icon: how many requests wait for the owner. The app exposes
// `window.agencyNative.setBadge(count)` (docs/apple.md); outside the app there is nothing to call.

/** The largest number the app accepts; anything above it is shown as this. */
export const MAX_BADGE = 9999

/** A whole number from 0 to 9999, which is all the app takes. */
export const badgeOf = (waiting: number): number => Math.min(MAX_BADGE, Math.max(0, Math.trunc(waiting)))

/** Sets the app icon's badge; does nothing in a browser, and never lets the bridge break the page. */
export function setNativeBadge(page: object, waiting: number): void {
  const native = (page as { agencyNative?: { setBadge?: unknown } }).agencyNative
  if (typeof native?.setBadge !== 'function') return
  try {
    native.setBadge.call(native, badgeOf(waiting))
  } catch {
    // The badge is a convenience; a failing bridge must not take the page down with it.
  }
}
