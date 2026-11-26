// The service worker of the installed app. It does three things and nothing else:
// - It keeps the built files of the app (/assets/ and /icons/: public code and pictures, hashed names) so that the app opens
//   fast; it never keeps, or answers, anything of the API or the federation (/api/, /fed/), a request that is not a plain GET,
//   or a page. The pages carry the node's write token, so every navigation goes to the network.
// - When the network fails for a navigation, it shows /offline.html, a static page that says the node cannot be reached. A
//   redirect to the sign-in (Access) is not a failure: it passes through untouched, so an expired sign-in is never hidden.
// - When the page says the viewer signed out (`signed-out`), it deletes what it kept.
const SHELL = 'agency-shell-1'
const ASSETS = 'agency-assets-1'
const OFFLINE = '/offline.html'
/** How many built files are kept: the old ones of earlier builds are dropped, oldest first. */
const MAX_ASSETS = 120

const isBuiltFile = (pathname) => pathname.startsWith('/assets/') || pathname.startsWith('/icons/')

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
  if (kept !== undefined) return kept
  const response = await fetch(request)
  if (response.ok && response.type === 'basic') {
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
