import { installFakeNotification, setPageVisibility, type FakeNotifications } from '../test/fakeNotification'
import { isOsNoticeOn } from './osChoice'
import { navigationOf, shouldTellOs, showOsNotice } from './osNotice'

describe('how a notice reaches the operating system', () => {
  let fake: FakeNotifications
  beforeEach(() => {
    fake = installFakeNotification('granted')
    window.localStorage.setItem('agency.osNotices', 'on')
    setPageVisibility('hidden')
  })
  afterEach(() => {
    fake.restore()
    setPageVisibility('visible')
  })

  it('tells the operating system only when the viewer asked, the browser allows it, and the page is hidden', () => {
    expect(shouldTellOs()).toBe(true)
    setPageVisibility('visible')
    expect(shouldTellOs()).toBe(false)
    setPageVisibility('hidden')
    fake.setPermission('denied')
    expect(shouldTellOs()).toBe(false)
    fake.setPermission('granted')
    window.localStorage.removeItem('agency.osNotices')
    expect(shouldTellOs()).toBe(false)
    expect(isOsNoticeOn()).toBe(false)
  })

  it('opens one request on the desk by its key, and every other target by its page', () => {
    expect(navigationOf({ to: '/desk', item: 'acme:app:r-1' })).toEqual({ to: '/desk', search: { item: 'acme:app:r-1' } })
    expect(navigationOf({ to: '/desk' })).toEqual({ to: '/desk' })
    expect(navigationOf({ to: '/people' })).toEqual({ to: '/people' })
    expect(navigationOf({ to: '/devices' })).toEqual({ to: '/devices' })
  })

  it('says the notice in its own words under the app’s name, and opens its target when it is clicked', () => {
    const opened: unknown[] = []
    const focus = vi.spyOn(window, 'focus').mockImplementation(() => undefined)
    showOsNotice({ tone: 'info', message: 'A request waits for you: Plan (app).', plain: 'A request is waiting for you', target: { to: '/desk', item: 'acme:app:r-1' } }, 'Agency', (target) => opened.push(target), { isTitleShown: true })
    expect(fake.shown[0]?.title).toBe('Agency')
    expect(fake.shown[0]?.options?.body).toBe('A request waits for you: Plan (app).')
    fake.shown[0]?.onclick?.()
    expect(focus).toHaveBeenCalled()
    expect(fake.shown[0]?.closed).toBe(true)
    expect(opened).toEqual([{ to: '/desk', item: 'acme:app:r-1' }])
  })

  it('says the words with no title when titles may not leave the page, and the words with the title when they may', () => {
    const notice = { tone: 'info' as const, message: 'A request waits for you: Plan (app).', plain: 'A request is waiting for you', target: { to: '/desk' as const } }
    showOsNotice(notice, 'Agency', () => undefined, { isTitleShown: false })
    showOsNotice(notice, 'Agency', () => undefined, { isTitleShown: true })
    expect(fake.shown.map((one) => one.options?.body)).toEqual(['A request is waiting for you', 'A request waits for you: Plan (app).'])
  })

  it('gives the browser the tag of the notice, and none when the notice has none', () => {
    showOsNotice({ tone: 'info', message: 'a', plain: 'a', target: { to: '/desk' }, tag: 'request/acme/app/r-1' }, 'Agency', () => undefined, { isTitleShown: true })
    showOsNotice({ tone: 'info', message: 'b', plain: 'b', target: { to: '/desk' } }, 'Agency', () => undefined, { isTitleShown: true })
    expect(fake.shown[0]?.options?.tag).toBe('request/acme/app/r-1')
    expect(fake.shown[1]?.options).not.toHaveProperty('tag')
  })

  it('does not fail the page when the browser cannot make a notification from it', () => {
    Object.defineProperty(globalThis, 'Notification', {
      value: class {
        static permission = 'granted'
        constructor() {
          throw new TypeError('Illegal constructor')
        }
      },
      configurable: true,
      writable: true,
    })
    expect(() => showOsNotice({ tone: 'info', message: 'x', plain: 'x', target: { to: '/desk' } }, 'Agency', () => undefined, { isTitleShown: true })).not.toThrow()
  })
})
