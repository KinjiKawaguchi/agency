/** Whether this page can have a service worker: a built page in a secure context (https, or this machine), never the dev server or a test. */
const canRegister = (): boolean => import.meta.env.PROD && typeof navigator !== 'undefined' && 'serviceWorker' in navigator && window.isSecureContext

/** Registers the service worker once the page has loaded, so that it does not compete with the page for the network. */
export function registerServiceWorker(): void {
  if (!canRegister()) return
  window.addEventListener('load', () => void navigator.serviceWorker.register('/sw.js', { scope: '/' }).catch(() => undefined))
}

/** Tells the service worker that the viewer signed out, so that it deletes the files it kept. */
export function forgetKeptFiles(): void {
  if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return
  navigator.serviceWorker.controller?.postMessage({ type: 'signed-out' })
}
