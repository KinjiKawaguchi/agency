import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { type FakeWindow, startWorker } from '../test/fakeWorker'

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
    expect((await pushed(payload(hidden), [], 'ja-JP')).shown[0]).toMatchObject({ title: 'Agency', options: { body: 'あなたを待っている依頼があります' } })
  })

  it('words a device and a refused decision by kind, with the node’s label after it, and tags them as the page does', async () => {
    const device = (await pushed(payload({ agency: { type: 'device' }, detail: 'Pixel' }))).shown[0]
    expect(device).toMatchObject({ title: 'Agency', options: { body: 'Device “Pixel” is waiting to be added', tag: 'device', data: { url: '/devices' } } })
    const decision = (await pushed(payload({ agency: { type: 'decision', org: 'acme', team: 'app', id: 'dec-1' } }))).shown[0]
    expect(decision).toMatchObject({ title: 'Agency', options: { body: 'A decision was refused', tag: 'decision/dec-1', data: { url: '/desk' } } })
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

  it('words each kind as the iPhone’s alert does, in both languages, from its own strings file', async () => {
    const strings = (language: 'en' | 'ja'): Map<string, string> => {
      const text = readFileSync(join(process.cwd(), '..', 'apple', 'Shared', `${language}.lproj`, 'Localizable.strings'), 'utf8')
      return new Map([...text.matchAll(/^"([A-Z_]+)" = "(.*)";$/gm)].map((found) => [found[1] ?? '', found[2] ?? '']))
    }
    const named = (template: string | undefined, text: string): string => (template ?? '').replace('%@', text)
    for (const [language, tag] of [['en', 'en-US'], ['ja', 'ja-JP']] as const) {
      const phone = strings(language)
      const body = async (agency: unknown, extra: object = {}) => (await pushed(payload({ agency, ...extra }), [], tag)).shown[0]?.options.body
      expect(await body({ org: 'acme', team: 'app', request: 'r-1' }), `${language} request`).toBe(phone.get('PUSH_REQUEST_WAITING_HIDDEN'))
      expect(await body({ type: 'device' }), `${language} device`).toBe(phone.get('PUSH_DEVICE_WAITING_NO_LABEL'))
      expect(await body({ type: 'device' }, { detail: 'Pixel' }), `${language} named device`).toBe(named(phone.get('PUSH_DEVICE_WAITING'), 'Pixel'))
      expect(await body({ type: 'decision', org: 'acme', team: 'app', id: 'd-1' }), `${language} decision`).toBe(phone.get('PUSH_DECISION_REFUSED_NO_TITLE'))
      expect(await body({ type: 'decision', org: 'acme', team: 'app', id: 'd-1' }, { detail: 'Ship it' }), `${language} named decision`).toBe(named(phone.get('PUSH_DECISION_REFUSED'), 'Ship it'))
    }
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

  it('opens a window at the request when the one in front cannot be taken there', async () => {
    const stuck = window({ focused: true, navigate: () => Promise.reject(new Error('not controlled')) })
    const { worker } = await tapped('/desk?item=acme%3Aapp%3Ar-1', [stuck])
    expect([stuck.focusCalls, worker.opened]).toEqual([1, ['/desk?item=acme%3Aapp%3Ar-1']])
    const noNavigate = window({ focused: true })
    delete (noNavigate as { navigate?: unknown }).navigate
    expect((await tapped('/devices', [noNavigate])).worker.opened).toEqual(['/devices'])
  })

  it('opens only a path of the app: anything else leads to the desk', async () => {
    for (const bad of ['https://evil.example/', '//evil.example/x', '/\\evil.example/x', '/\t/evil.example/x', '/\n//evil.example', '/ /evil', '/desk\u0001x', '/desk\u007f', '/desk\u001f?item=1', 'javascript:alert(1)', undefined, 7, '']) {
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

describe('a recovery of an org that was started', () => {
  const recovery = { agency: { type: 'recovery', org: 'acme' }, badge: 0, title: 'A recovery of acme was started', body: 'It takes effect at Dec 1, 9:30 AM. If you did not start it, sign any version of the org now.' }
  const START = '2026-12-01T09:30:00Z'
  const FINISH = '2026-12-04T09:30:00Z'
  const shownAt = (iso: string, language: string): string => new Intl.DateTimeFormat(language, { month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }).format(new Date(iso))

  it('is worded by the page in its own words, not the node’s, tagged by the org, and leads to the desk, where the notice is', async () => {
    expect((await pushed(payload(recovery))).shown).toEqual([{ title: 'Agency', options: { body: 'A recovery of acme was started. Open the desk to see it.', icon: '/icons/icon-192.png', tag: 'recovery/acme', data: { url: '/desk' } } }])
  })

  it('says it in Japanese', async () => {
    expect((await pushed(payload(recovery), [], 'ja-JP')).shown[0]).toMatchObject({ title: 'Agency', options: { body: 'acme のルート鍵の回復が始まりました。机で確認してください。', tag: 'recovery/acme' } })
  })

  it('carries the times, in the viewer’s locale, when the node gives them', async () => {
    const timed = { agency: { ...recovery.agency, startedAt: START, finishesAt: FINISH }, badge: 0 }
    expect((await pushed(payload(timed), [], 'en-US')).shown[0]?.options.body).toBe(`A recovery of acme was started at ${shownAt(START, 'en-US')}. It takes effect at ${shownAt(FINISH, 'en-US')}. Open the desk to see it.`)
    expect((await pushed(payload(timed), [], 'ja-JP')).shown[0]?.options.body).toBe(`acme のルート鍵の回復が始まりました（${shownAt(START, 'ja-JP')}）。${shownAt(FINISH, 'ja-JP')} に有効になります。机で確認してください。`)
  })

  it('leaves out a time that is not one, and says the one that is', async () => {
    const half = { agency: { ...recovery.agency, startedAt: 'yesterday', finishesAt: FINISH }, badge: 0 }
    expect((await pushed(payload(half), [], 'en-US')).shown[0]?.options.body).toBe(`A recovery of acme was started. It takes effect at ${shownAt(FINISH, 'en-US')}. Open the desk to see it.`)
    for (const bad of [7, null, '', {}]) expect((await pushed(payload({ agency: { ...recovery.agency, finishesAt: bad }, badge: 0 }))).shown[0]?.options.body, String(bad)).toBe('A recovery of acme was started. Open the desk to see it.')
  })

  it('names no org, and has no tag, when the id is not a plain one', async () => {
    const odd = { agency: { type: 'recovery', org: 'a/b' }, badge: 0 }
    expect((await pushed(payload(odd))).shown[0]).toMatchObject({ title: 'Agency', options: { body: 'A recovery of an org was started. Open the desk to see it.', data: { url: '/desk' } } })
    expect((await pushed(payload(odd))).shown[0]?.options.tag).toBeUndefined()
  })

  it('is not taken for a request, whose address it does not have', async () => {
    expect((await pushed(payload({ agency: { type: 'recovery', org: 'acme', team: 'app', request: 'r-1' }, badge: 0 }))).shown[0]?.options.data.url).toBe('/desk')
  })
})

describe('a change that waits for the root key', () => {
  const held = { agency: { type: 'root', org: 'acme', id: '0123456789abcdef', waiting: 1 }, badge: 0, title: 'A change to acme waits for the root key', body: 'add member carol (Carol). Run agency root-sign on the root key\'s node to sign it.' }

  it('is worded by the page in its own words, tagged by the change’s id, and leads to the desk, where it is an item', async () => {
    expect((await pushed(payload(held))).shown).toEqual([{ title: 'Agency', options: { body: 'A change to acme waits for the root key’s signature. Open the desk to see it.', icon: '/icons/icon-192.png', tag: 'root/0123456789abcdef', data: { url: '/desk' } } }])
  })

  it('does not say the node’s English, which names a member or a device by its label', async () => {
    expect(JSON.stringify((await pushed(payload(held))).shown)).not.toMatch(/carol|Carol|add member/)
  })

  it('says how many wait when more than one does', async () => {
    const many = { agency: { ...held.agency, waiting: 3 }, badge: 0 }
    expect((await pushed(payload(many))).shown[0]?.options.body).toBe('3 changes to acme wait for the root key’s signature. Open the desk to see them.')
    expect((await pushed(payload(many), [], 'ja-JP')).shown[0]?.options.body).toBe('acme の変更 3 件が、ルート鍵の署名を待っています。机で確認してください。')
    for (const bad of [0, -2, 1.5, '3', null]) expect((await pushed(payload({ agency: { ...held.agency, waiting: bad }, badge: 0 }))).shown[0]?.options.body, String(bad)).toBe('A change to acme waits for the root key’s signature. Open the desk to see it.')
  })

  it('says it in Japanese', async () => {
    expect((await pushed(payload(held), [], 'ja-JP')).shown[0]).toMatchObject({ title: 'Agency', options: { body: 'acme の変更が、ルート鍵の署名を待っています。机で確認してください。', tag: 'root/0123456789abcdef' } })
  })

  it('names no org, and has no tag, when the identifiers are not plain ones', async () => {
    const odd = { agency: { type: 'root', org: 'a/b', id: 'x y', waiting: 1 }, badge: 0 }
    const [shown] = (await pushed(payload(odd))).shown
    expect(shown?.options.body).toBe('A change waits for the root key’s signature. Open the desk to see it.')
    expect(shown?.options.tag).toBeUndefined()
    expect(shown?.options.data.url).toBe('/desk')
  })
})
