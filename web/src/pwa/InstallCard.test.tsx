import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { I18nProvider } from '../i18n'
import { InstallCard } from './InstallCard'

function renderCard() {
  return render(
    <I18nProvider initial="en">
      <InstallCard />
    </I18nProvider>,
  )
}

/** What a Chromium browser fires when the app can be installed. */
function offer(outcome: 'accepted' | 'dismissed' = 'accepted') {
  const event = Object.assign(new Event('beforeinstallprompt', { cancelable: true }), { prompt: vi.fn(() => Promise.resolve()), userChoice: Promise.resolve({ outcome }) })
  act(() => void window.dispatchEvent(event))
  return event
}

describe('the install card', () => {
  const originalMatchMedia = window.matchMedia
  afterEach(() => void (window.matchMedia = originalMatchMedia))

  it('is not there until the browser offers to install the app', () => {
    renderCard()
    expect(screen.queryByRole('button', { name: 'Install Agency' })).toBeNull()
    offer()
    expect(screen.getByRole('button', { name: 'Install Agency' })).toBeInTheDocument()
  })

  it('holds the browser’s own prompt back, and shows it only when the viewer asks', async () => {
    const user = userEvent.setup()
    renderCard()
    const event = offer()
    expect(event.defaultPrevented).toBe(true)
    expect(event.prompt).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: 'Install Agency' }))
    expect(event.prompt).toHaveBeenCalledTimes(1)
  })

  it('goes away once the offer has been used, accepted or not, since an offer is used once', async () => {
    const user = userEvent.setup()
    renderCard()
    offer('dismissed')
    await user.click(screen.getByRole('button', { name: 'Install Agency' }))
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Install Agency' })).toBeNull())
  })

  it('is not shown in the installed app itself, and goes away when the app gets installed', () => {
    window.matchMedia = ((query: string) => ({ matches: query.includes('standalone'), media: query, addEventListener: () => undefined, removeEventListener: () => undefined })) as unknown as typeof window.matchMedia
    renderCard()
    offer()
    expect(screen.queryByRole('button', { name: 'Install Agency' })).toBeNull()
  })

  it('goes away when the app is installed from the browser’s own menu', () => {
    renderCard()
    offer()
    expect(screen.getByRole('button', { name: 'Install Agency' })).toBeInTheDocument()
    act(() => void window.dispatchEvent(new Event('appinstalled')))
    expect(screen.queryByRole('button', { name: 'Install Agency' })).toBeNull()
  })
})
