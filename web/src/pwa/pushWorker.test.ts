import { describe, expect, it } from 'vitest'
import { type FakeWindow, startWorker } from './fakeWorker'

const network = () => Promise.resolve(new Response('x'))
const payload = (value: unknown): string => JSON.stringify(value)
const request = { agency: { org: 'acme', team: 'app', request: 'r-1' }, badge: 2, title: 'App team', body: 'Ship the release?' }

const window = (overrides: Partial<FakeWindow> = {}): FakeWindow & { focusCalls: number; navigated: string[]; messages: unknown[] } => {
  const state = { focusCalls: 0, navigated: [] as string[], messages: [] as unknown[] }
  return {
    focused: false,
    visibilityState: 'visible',
    focus: () => (state.focusCalls++, Promise.resolve()),
    navigate: (path: string) => (state.navigated.push(path), Promise.resolve()),
    postMessage: (message: unknown) => void state.messages.push(message),
    ...overrides,
    get focusCalls() {
      return state.focusCalls
    },
    get navigated() {
      return state.navigated
    },
    get messages() {
      return state.messages
    },
  }
}

async function pushed(data: string | undefined, windows: FakeWindow[] = [], language = 'en-US') {
  const worker = startWorker(network, { windows, language })
  await worker.run('push', data === undefined ? undefined : { text: () => data })
  return worker
}

describe('a notice pushed by the node', () => {
  it('is shown with the node’s words, a tag that names the request, and a quiet replacement', async () => {
    const worker = await pushed(payload(request))
    expect(worker.shown).toEqual([{ title: 'App team', options: { body: 'Ship the release?', icon: '/icons/icon-192.png', tag: 'request/acme/app/r-1', data: { url: '/desk?item=acme%3Aapp%3Ar-1' } } }])
    expect(worker.shown[0]?.options.renotify).toBeUndefined()
  })

  it('says only what kind it is when the node left the text out, in the viewer’s language', async () => {
    const hidden = { agency: request.agency, badge: 1 }
    expect((await pushed(payload(hidden))).shown[0]).toMatchObject({ title: 'Agency', options: { body: 'A request is waiting for you', tag: 'request/acme/app/r-1' } })
    expect((await pushed(payload(hidden), [], 'ja-JP')).shown[0]).toMatchObject({ title: 'Agency', options: { body: '依頼があなたを待っています' } })
  })

  it('words a device and a refused decision by kind, with the node’s label after it, and tags them as the page does', async () => {
    const device = (await pushed(payload({ agency: { type: 'device' }, detail: 'Pixel' }))).shown[0]
    expect(device).toMatchObject({ title: 'A device asks to be added', options: { body: 'Pixel', tag: 'device', data: { url: '/devices' } } })
    const decision = (await pushed(payload({ agency: { type: 'decision', org: 'acme', team: 'app', id: 'dec-1' } }))).shown[0]
    expect(decision).toMatchObject({ title: 'A decision of yours was refused', options: { body: '', tag: 'decision/dec-1', data: { url: '/desk' } } })
  })

  it('is shown as a plain notification when it is not one the app knows, and never fails', async () => {
    for (const data of [undefined, 'not json', '[]', '{}', payload({ agency: { type: 'telegram' } }), payload({ agency: null })]) {
      const worker = await pushed(data)
      expect(worker.shown).toEqual([{ title: 'Agency', options: { body: '', icon: '/icons/icon-192.png', data: { url: '/desk' } } }])
    }
  })

  it('leads to the desk, with no tag, when an identifier is not a plain one', async () => {
    for (const bad of ['a/b', 'a:b', '../x', 'a b', '', '.', '..', 'x'.repeat(201), 7]) {
      const worker = await pushed(payload({ ...request, agency: { ...request.agency, request: bad } }))
      expect(worker.shown[0]?.options.data.url, String(bad)).toBe('/desk')
      expect(worker.shown[0]?.options.tag, String(bad)).toBeUndefined()
    }
    const worker = await pushed(payload({ agency: { type: 'decision', org: 'acme', team: 'app', id: 'a/b' } }))
    expect(worker.shown[0]?.options.tag).toBeUndefined()
  })

  it('is shown while a window is hidden or not in front, and not while one is in front', async () => {
    for (const windows of [[], [window()], [window({ focused: true, visibilityState: 'hidden' })]]) expect((await pushed(payload(request), windows)).shown).toHaveLength(1)
    for (const windows of [[window({ focused: true })], [window(), window({ focused: true })]]) expect((await pushed(payload(request), windows)).shown).toHaveLength(0)
  })

  it('cuts a long title and body', async () => {
    const shown = (await pushed(payload({ ...request, title: 'T'.repeat(500), body: 'B'.repeat(500) }))).shown[0]
    expect([shown?.title.length, shown?.options.body.length]).toEqual([120, 240])
  })
})

describe('a tap on a notification', () => {
  async function tapped(url: unknown, windows: FakeWindow[]) {
    const worker = startWorker(network, { windows })
    let closed = false
    await worker.run('notificationclick', undefined, { notification: { close: () => void (closed = true), data: { url } } })
    return { worker, closed }
  }

  it('brings an open window forward and takes it to the request', async () => {
    const open = window()
    const { worker, closed } = await tapped('/desk?item=acme%3Aapp%3Ar-1', [open])
    expect([closed, open.focusCalls, open.navigated, worker.opened]).toEqual([true, 1, ['/desk?item=acme%3Aapp%3Ar-1'], []])
  })

  it('prefers the window that is in front, and opens one when there is none', async () => {
    const back = window()
    const front = window({ focused: true })
    await tapped('/devices', [back, front])
    expect([back.focusCalls, front.focusCalls]).toEqual([0, 1])
    expect((await tapped('/devices', [])).worker.opened).toEqual(['/devices'])
  })

  it('opens only a path of the app: anything else leads to the desk', async () => {
    for (const bad of ['https://evil.example/', '//evil.example/x', 'javascript:alert(1)', undefined, 7, '']) {
      expect((await tapped(bad, [])).worker.opened, String(bad)).toEqual(['/desk'])
    }
  })
})

describe('a subscription the browser ended', () => {
  it('is taken again with the same options, and the open windows are told so that the page registers it', async () => {
    const open = window()
    const worker = startWorker(network, { windows: [open] })
    const options = { userVisibleOnly: true, applicationServerKey: new Uint8Array([4, 1]) }
    await worker.run('pushsubscriptionchange', undefined, { oldSubscription: { options } })
    expect(worker.subscribed).toEqual([options])
    expect(open.messages).toEqual([{ type: 'push-subscription-changed' }])
  })

  it('tells the windows even when it has no old subscription to take again', async () => {
    const open = window()
    const worker = startWorker(network, { windows: [open] })
    await worker.run('pushsubscriptionchange')
    expect([worker.subscribed, open.messages]).toEqual([[], [{ type: 'push-subscription-changed' }]])
  })
})
