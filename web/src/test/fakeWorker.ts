import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import vm from 'node:vm'

// The worker and its notice module are the files the node serves, run in a fake worker scope.
const publicFile = (name: string): string => readFileSync(join(process.cwd(), 'public', name), 'utf8')
const source = publicFile('sw.js')
const noticeSource = publicFile('push-notice.js')

type Listener = (event: Record<string, unknown>) => void

/** A window of the app as the worker sees it. */
export interface FakeWindow {
  focused: boolean
  visibilityState: 'visible' | 'hidden'
  focus: () => Promise<void>
  navigate?: (path: string) => Promise<void>
  postMessage: (message: unknown) => void
}

export interface Shown {
  title: string
  options: { body: string; icon: string; tag?: string; renotify?: boolean; data: { url: string } }
}

export interface WorkerOptions {
  windows?: FakeWindow[]
  language?: string
}

export function startWorker(network: (request: Request) => Promise<Response>, { windows = [], language = 'en-US' }: WorkerOptions = {}) {
  const stores = new Map<string, Map<string, Response>>()
  const listeners = new Map<string, Listener>()
  const shown: Shown[] = []
  const opened: string[] = []
  const subscribed: unknown[] = []
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
  const self = {
    location: { origin: 'https://agency.example' },
    navigator: { language },
    addEventListener: (type: string, listener: Listener) => void listeners.set(type, listener),
    skipWaiting: () => Promise.resolve(),
    clients: { claim: () => Promise.resolve(), matchAll: () => Promise.resolve(windows), openWindow: (path: string) => (opened.push(path), Promise.resolve(undefined)) },
    registration: {
      showNotification: (title: string, options: Shown['options']) => (shown.push({ title, options }), Promise.resolve()),
      pushManager: { subscribe: (options: unknown) => (subscribed.push(options), Promise.resolve({})) },
    },
  }
  const importScripts = (path: string): void => {
    if (path !== '/push-notice.js') throw new Error(`the worker imports ${path}`)
    vm.runInNewContext(noticeSource, { self, globalThis: self })
  }
  vm.runInNewContext(source, { self, caches, fetch: network, URL, Promise, Math, importScripts, encodeURIComponent })

  /** Fires a fetch event; answers what the worker answered, or `undefined` when it left the request to the browser. */
  const fetchEvent = async (request: Request): Promise<Response | undefined> => {
    let answer: Promise<Response> | undefined
    listeners.get('fetch')?.({ request, respondWith: (response: Promise<Response>) => void (answer = response) })
    return answer
  }
  const run = async (type: string, data?: unknown, extra: Record<string, unknown> = {}): Promise<void> => {
    const waits: Promise<unknown>[] = []
    listeners.get(type)?.({ data, ...extra, waitUntil: (promise: Promise<unknown>) => void waits.push(promise) })
    await Promise.all(waits)
  }
  return { stores, fetchEvent, run, shown, opened, subscribed, listens: (type: string) => listeners.has(type) }
}
