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
    showOsNotice({ tone: 'info', message: 'A request waits for you.', target: { to: '/desk', item: 'acme:app:r-1' } }, 'Agency', (target) => opened.push(target))
    expect(fake.shown[0]?.title).toBe('Agency')
    expect(fake.shown[0]?.options?.body).toBe('A request waits for you.')
    fake.shown[0]?.onclick?.()
    expect(focus).toHaveBeenCalled()
    expect(fake.shown[0]?.closed).toBe(true)
    expect(opened).toEqual([{ to: '/desk', item: 'acme:app:r-1' }])
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
    expect(() => showOsNotice({ tone: 'info', message: 'x', target: { to: '/desk' } }, 'Agency', () => undefined)).not.toThrow()
  })
})
