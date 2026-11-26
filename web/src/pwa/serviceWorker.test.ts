import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import vm from 'node:vm'

// The worker is the file the node serves, run here in a fake worker scope: what it keeps and what it never touches.
const source = readFileSync(join(process.cwd(), 'public', 'sw.js'), 'utf8')

type Listener = (event: Record<string, unknown>) => void

function startWorker(network: (request: Request) => Promise<Response>) {
  const stores = new Map<string, Map<string, Response>>()
  const listeners = new Map<string, Listener>()
  const keyOf = (request: Request | string): string => (typeof request === 'string' ? new URL(request, 'https://agency.example').href : request.url)
  const cacheOf = (name: string) => {
    const store = stores.get(name) ?? new Map<string, Response>()
    stores.set(name, store)
    return {
      match: (request: Request | string) => Promise.resolve(store.get(keyOf(request))?.clone()),
      put: (request: Request, response: Response) => (store.set(keyOf(request), response), Promise.resolve()),
      addAll: async (paths: string[]) => void paths.forEach((path) => store.set(keyOf(path), new Response(`kept ${path}`))),
      keys: () => Promise.resolve([...store.keys()].map((url) => new Request(url))),
      delete: (request: Request) => Promise.resolve(store.delete(keyOf(request))),
    }
  }
  const caches = {
    open: (name: string) => Promise.resolve(cacheOf(name)),
    match: async (request: Request | string) => {
      for (const store of stores.values()) if (store.has(keyOf(request))) return store.get(keyOf(request))?.clone()
      return undefined
    },
    keys: () => Promise.resolve([...stores.keys()]),
    delete: (name: string) => Promise.resolve(stores.delete(name)),
  }
  const self = { location: { origin: 'https://agency.example' }, addEventListener: (type: string, listener: Listener) => void listeners.set(type, listener), skipWaiting: () => Promise.resolve(), clients: { claim: () => Promise.resolve() } }
  vm.runInNewContext(source, { self, caches, fetch: network, URL, Promise, Math })

  /** Fires a fetch event; answers what the worker answered, or `undefined` when it left the request to the browser. */
  const fetchEvent = async (request: Request): Promise<Response | undefined> => {
    let answer: Promise<Response> | undefined
    listeners.get('fetch')?.({ request, respondWith: (response: Promise<Response>) => void (answer = response) })
    return answer
  }
  const run = async (type: string, data?: unknown): Promise<void> => {
    const waits: Promise<unknown>[] = []
    listeners.get(type)?.({ data, waitUntil: (promise: Promise<unknown>) => void waits.push(promise) })
    await Promise.all(waits)
  }
  return { stores, fetchEvent, run }
}

/** An answer as the network gives for this origin: its type is `basic`, which a hand-made Response does not have. */
const basic = (body: string, init: ResponseInit = {}): Response => Object.defineProperty(new Response(body, init), 'type', { value: 'basic' })
const get = (path: string, init: RequestInit = {}): Request => new Request(`https://agency.example${path}`, init)
const navigation = (path: string): Request => Object.defineProperty(get(path), 'mode', { value: 'navigate' })

describe('the service worker', () => {
  it('keeps the built files, once, and serves them from what it kept', async () => {
    let calls = 0
    const worker = startWorker(() => (++calls, Promise.resolve(basic('console.log(1)'))))
    const first = await worker.fetchEvent(get('/assets/index-abc.js'))
    expect(await first?.text()).toBe('console.log(1)')
    const again = await worker.fetchEvent(get('/assets/index-abc.js'))
    expect(await again?.text()).toBe('console.log(1)')
    expect(calls).toBe(1)
  })

  it('never touches the API, the federation, a write, a page of the app, or another origin', async () => {
    const worker = startWorker(() => Promise.resolve(new Response('x')))
    for (const path of ['/api/me', '/api/orgs/acme/teams/app/view', '/fed/v1/teams/acme/app', '/people', '/manifest.webmanifest', '/sw.js']) {
      expect(await worker.fetchEvent(get(path))).toBeUndefined()
    }
    expect(await worker.fetchEvent(get('/assets/index.js', { method: 'POST', body: 'x' }))).toBeUndefined()
    expect(await worker.fetchEvent(new Request('https://elsewhere.example/assets/index.js'))).toBeUndefined()
    const names = [...worker.stores.keys()]
    for (const store of worker.stores.values()) for (const url of store.keys()) expect(url).not.toMatch(/\/(api|fed)\//)
    expect(names.length).toBeLessThanOrEqual(2)
  })

  it('does not keep an answer that is not a file of this origin or not a success', async () => {
    const worker = startWorker((request) => Promise.resolve(request.url.includes('gone') ? basic('no', { status: 404 }) : Response.redirect('https://login.example/', 302)))
    expect((await (await worker.fetchEvent(get('/assets/gone.js')))?.text()) ?? '').toBe('no')
    await worker.fetchEvent(get('/assets/moved.js')).catch(() => undefined)
    for (const store of worker.stores.values()) expect([...store.keys()].some((url) => url.includes('/assets/'))).toBe(false)
  })

  it('sends a page to the network, and shows the offline page only when the network fails', async () => {
    let isUp = true
    const worker = startWorker(() => (isUp ? Promise.resolve(new Response('the page with the write token')) : Promise.reject(new TypeError('Failed to fetch'))))
    await worker.run('install')
    expect(await (await worker.fetchEvent(navigation('/desk')))?.text()).toBe('the page with the write token')
    // A page is never kept: after the network goes, the answer is the offline page and not the page it saw.
    isUp = false
    expect(await (await worker.fetchEvent(navigation('/desk')))?.text()).toBe('kept /offline.html')
    for (const store of worker.stores.values()) expect([...store.values()].every((response) => !response.bodyUsed)).toBe(true)
  })

  it('does not hide a redirect to the sign-in: it passes through', async () => {
    const redirect = Response.redirect('https://team.cloudflareaccess.com/login', 302)
    const worker = startWorker(() => Promise.resolve(redirect))
    expect((await worker.fetchEvent(navigation('/desk')))?.status).toBe(302)
  })

  it('drops what earlier versions kept when it takes over, and what it kept when the viewer signed out', async () => {
    const worker = startWorker(() => Promise.resolve(basic('file')))
    worker.stores.set('agency-assets-0', new Map([['https://agency.example/assets/old.js', new Response('old')]]))
    await worker.run('install')
    await worker.run('activate')
    expect([...worker.stores.keys()].toSorted()).toEqual(['agency-shell-1'])
    await worker.fetchEvent(get('/assets/new.js'))
    expect(worker.stores.has('agency-assets-1')).toBe(true)
    await worker.run('message', { type: 'signed-out' })
    expect(worker.stores.has('agency-assets-1')).toBe(false)
    expect(worker.stores.has('agency-shell-1')).toBe(true)
  })

  it('keeps a bounded number of files', async () => {
    const worker = startWorker((request) => Promise.resolve(basic(request.url)))
    for (let index = 0; index < 150; index += 1) await worker.fetchEvent(get(`/assets/chunk-${index}.js`))
    expect(worker.stores.get('agency-assets-1')?.size).toBe(120)
  })
})
