import { act, renderHook, waitFor } from '@testing-library/react'
import { installFakeNotification, type FakeNotifications } from '../test/fakeNotification'
import { useOsNotices } from './osChoice'

describe('the choice follows what the browser allows', () => {
  let fake: FakeNotifications
  let status: EventTarget
  beforeEach(() => {
    fake = installFakeNotification('denied')
    status = new EventTarget()
    Object.defineProperty(navigator, 'permissions', { value: { query: () => Promise.resolve(status) }, configurable: true })
  })
  afterEach(() => {
    fake.restore()
    Reflect.deleteProperty(navigator, 'permissions')
  })

  it('notices that the person allowed them in the browser’s settings, and that they took it back', async () => {
    window.localStorage.setItem('agency.osNotices', 'on')
    const { result } = renderHook(() => useOsNotices())
    expect(result.current).toMatchObject({ support: 'denied', isOn: false })
    await act(async () => {
      await Promise.resolve()
    })
    act(() => {
      fake.setPermission('granted')
      status.dispatchEvent(new Event('change'))
    })
    await waitFor(() => expect(result.current).toMatchObject({ support: 'granted', isOn: true }))
    act(() => {
      fake.setPermission('denied')
      status.dispatchEvent(new Event('change'))
    })
    await waitFor(() => expect(result.current).toMatchObject({ support: 'denied', isOn: false }))
  })

  it('stops listening when it is no longer shown', async () => {
    const listening = new Set<unknown>()
    status.addEventListener = ((type: string, listener: unknown) => void (type === 'change' && listening.add(listener))) as EventTarget['addEventListener']
    status.removeEventListener = ((type: string, listener: unknown) => void (type === 'change' && listening.delete(listener))) as EventTarget['removeEventListener']
    const { unmount } = renderHook(() => useOsNotices())
    await waitFor(() => expect(listening.size).toBe(1))
    unmount()
    expect(listening.size).toBe(0)
  })
})
