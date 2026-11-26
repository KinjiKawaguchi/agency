// The service worker of the installed app. It does three things and nothing else:
// - It keeps the built files of the app (/assets/ and /icons/: public code and pictures, hashed names) so that the app opens
//   fast; it never keeps, or answers, anything of the API or the federation (/api/, /fed/), a request that is not a plain GET,
//   or a page. The pages carry the node's write token, so every navigation goes to the network.
// - When the network fails for a navigation, it shows /offline.html, a static page that says the node cannot be reached. A
//   redirect to the sign-in (Access) is not a failure: it passes through untouched, so an expired sign-in is never hidden.
// - When the page says the viewer signed out (`signed-out`), it deletes what it kept.
// - When the node pushes a notice (`push`), it shows it, unless a window of the app is in front, where the page itself tells what
//   is new. What the notice says and where a tap goes is decided in /push-notice.js, which this file only calls (it is tested
//   there); a tap brings an open window of the app forward, or opens one. A notification never makes a sound again when it
//   replaces another about the same thing (no `renotify`).
// It holds no state of its own, which is why a new worker may take over the open pages at once (`skipWaiting`, `claim`).
// It cannot rely on the node to answer a missing built file with a 404: the node answers any path it does not know with the
// page (a 200 `text/html` that carries the write token), so a file is kept only when it is a file of the kind that was asked for.
importScripts('/push-notice.js')
const notice = self.AgencyNotice

const SHELL = 'agency-shell-1'
const ASSETS = 'agency-assets-1'
const OFFLINE = '/offline.html'
/** How many built files are kept: the old ones of earlier builds are dropped, oldest first. */
const MAX_ASSETS = 120

const isBuiltFile = (pathname) => pathname.startsWith('/assets/') || pathname.startsWith('/icons/')

/** What a built file is for: a script, a style sheet, a picture, a font (an empty destination is a call of `fetch()`). */
const KEPT_DESTINATIONS = ['script', 'style', 'image', 'font', '']

const isPage = (response) => (response.headers.get('content-type') || '').toLowerCase().startsWith('text/html')

/** Whether an answer is a file worth keeping: a success of this origin, not a page, and of the kind asked for. */
const isKeepable = (request, response) => response.ok && response.type === 'basic' && !isPage(response) && (response.headers.get('content-type') || '') !== '' && KEPT_DESTINATIONS.includes(request.destination)

self.addEventListener('install', (event) => {
  event.waitUntil(caches.open(SHELL).then((cache) => cache.addAll([OFFLINE, '/offline.css'])).then(() => self.skipWaiting()))
})

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((name) => name !== SHELL && name !== ASSETS).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  )
})

async function trim(cache) {
  const keys = await cache.keys()
  await Promise.all(keys.slice(0, Math.max(0, keys.length - MAX_ASSETS)).map((key) => cache.delete(key)))
}

async function fromCacheOrNetwork(request) {
  const cache = await caches.open(ASSETS)
  const kept = await cache.match(request)
  // A page that was kept by mistake (by an earlier worker) is dropped, not served.
  if (kept !== undefined && !isPage(kept)) return kept
  if (kept !== undefined) await cache.delete(request)
  const response = await fetch(request)
  if (isKeepable(request, response)) {
    await cache.put(request, response.clone())
    await trim(cache)
  }
  return response
}

self.addEventListener('fetch', (event) => {
  const { request } = event
  if (request.method !== 'GET') return
  const url = new URL(request.url)
  if (url.origin !== self.location.origin) return
  if (request.mode === 'navigate') {
    event.respondWith(fetch(request).catch(() => caches.match(OFFLINE)))
    return
  }
  if (isBuiltFile(url.pathname)) event.respondWith(fromCacheOrNetwork(request))
})

self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'signed-out') event.waitUntil(caches.delete(ASSETS))
})

const windowsOfTheApp = () => self.clients.matchAll({ type: 'window', includeUncontrolled: true })

async function showNotice(data) {
  const content = notice.contentOf(data, self.navigator && self.navigator.language)
  if (!notice.shouldShow(await windowsOfTheApp())) return
  const options = { body: content.body, icon: '/icons/icon-192.png', data: { url: content.url } }
  if (content.tag !== undefined) options.tag = content.tag
  await self.registration.showNotification(content.title, options)
}

self.addEventListener('push', (event) => {
  event.waitUntil(showNotice(event.data ? event.data.text() : undefined))
})

/** A path of the app and nothing else (never another origin, never `//host`), whatever the notification carried. */
const pathOf = (value) => (typeof value === 'string' && /^\/(?!\/)/.test(value) ? value : '/desk')

async function openOrFocus(path) {
  const one = notice.windowToFocus(await windowsOfTheApp())
  if (one !== undefined) {
    await one.focus()
    if (typeof one.navigate === 'function') await one.navigate(path).catch(() => undefined)
    return
  }
  await self.clients.openWindow(path)
}

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  event.waitUntil(openOrFocus(pathOf(event.notification.data && event.notification.data.url)))
})

// The browser ended a subscription (its push service renewed it). The worker has no write token to tell the node, so it takes
// a new one with the same options and tells the open windows; the page registers it at the node, and does so when it next opens.
self.addEventListener('pushsubscriptionchange', (event) => {
  event.waitUntil(
    (async () => {
      const options = event.oldSubscription ? event.oldSubscription.options : undefined
      if (options !== undefined) await self.registration.pushManager.subscribe(options).catch(() => undefined)
      for (const one of await windowsOfTheApp()) one.postMessage({ type: 'push-subscription-changed' })
    })(),
  )
})
