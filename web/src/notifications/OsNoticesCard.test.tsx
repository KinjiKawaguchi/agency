import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { installFakeNotification, type FakeNotifications } from '../test/fakeNotification'
import { renderApp } from '../test/render'

describe('the switch for notifications of the operating system', () => {
  let fake: FakeNotifications
  afterEach(() => fake.restore())

  it('asks the browser for the permission when it is clicked, and is on once it is given, and stays on after a reload', async () => {
    fake = installFakeNotification('default')
    const user = userEvent.setup()
    const { unmount } = renderApp({ path: '/devices', enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Turn on notifications' }))
    expect(fake.asked()).toBe(1)
    expect(await screen.findByRole('button', { name: 'Turn off notifications' })).toBeInTheDocument()
    expect(screen.getByText(/A notification appears when something is new while this page is hidden/)).toBeInTheDocument()
    expect(window.localStorage.getItem('agency.osNotices')).toBe('on')
    unmount()
    renderApp({ path: '/devices', enrolled: true })
    expect(await screen.findByRole('button', { name: 'Turn off notifications' })).toBeInTheDocument()
  })

  it('asks nothing of the browser before it is clicked', async () => {
    fake = installFakeNotification('default')
    renderApp({ path: '/devices', enrolled: true })
    await screen.findByRole('button', { name: 'Turn on notifications' })
    expect(fake.asked()).toBe(0)
  })

  it('stays off and says why when the person refuses the permission', async () => {
    fake = installFakeNotification('default')
    fake.answer('denied')
    const user = userEvent.setup()
    renderApp({ path: '/devices', enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Turn on notifications' }))
    expect(await screen.findByText(/does not allow notifications for this page/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /notifications$/ })).not.toBeInTheDocument()
    expect(window.localStorage.getItem('agency.osNotices')).toBeNull()
  })

  it('says the browser cannot, and offers no switch, when it has no notifications', async () => {
    fake = installFakeNotification('default')
    fake.restore()
    Object.defineProperty(globalThis, 'Notification', { value: undefined, configurable: true, writable: true })
    renderApp({ path: '/devices', enrolled: true })
    expect(await screen.findByText('This browser cannot show notifications from a page.')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Turn on notifications' })).not.toBeInTheDocument()
  })

  it('turns off when the switch is clicked, and forgets the choice', async () => {
    fake = installFakeNotification('granted')
    window.localStorage.setItem('agency.osNotices', 'on')
    const user = userEvent.setup()
    renderApp({ path: '/devices', enrolled: true })
    await user.click(await screen.findByRole('button', { name: 'Turn off notifications' }))
    await waitFor(() => expect(window.localStorage.getItem('agency.osNotices')).toBeNull())
    expect(await screen.findByRole('button', { name: 'Turn on notifications' })).toBeInTheDocument()
  })
})
