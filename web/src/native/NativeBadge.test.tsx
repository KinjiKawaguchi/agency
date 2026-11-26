import { screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { createTestNode } from '../test/testDevice'
import { renderApp } from '../test/render'

type Native = { platform: string; push: null; setBadge: ReturnType<typeof vi.fn> }

function installApp(): Native {
  const native: Native = { platform: 'ios', push: null, setBadge: vi.fn() }
  ;(window as unknown as { agencyNative?: unknown }).agencyNative = native
  return native
}

afterEach(() => {
  delete (window as unknown as { agencyNative?: unknown }).agencyNative
})

const counts = (native: Native): number[] => native.setBadge.mock.calls.map(([count]) => count as number)

describe('the app icon’s badge', () => {
  it('shows the number of requests waiting for the owner, the same number the desk’s tab shows, and never a 0 on the way', async () => {
    const native = installApp()
    renderApp()
    await waitFor(() => expect(counts(native)).toContain(3))
    expect(screen.getByRole('link', { name: /Decisions/ })).toHaveTextContent('3')
    expect(counts(native).every((count) => count === 3)).toBe(true)
  })

  it('does not count what waits on someone else', async () => {
    const native = installApp()
    const { withRequestFor } = await import('../test/views')
    renderApp({ view: withRequestFor('frontend') })
    await screen.findByRole('listbox', { name: 'Documents waiting' })
    await waitFor(() => expect(counts(native).length).toBeGreaterThan(0))
    expect(new Set(counts(native))).toEqual(new Set([3]))
  })

  it('counts what the node lists as the owner’s, not what `to` seems to say', async () => {
    const native = installApp()
    const node = createTestNode()
    const real = createApiClient((request) => node.handle(request))
    // The node lists one request fewer as the viewer's, as it would for a request it does not map to them.
    const client = {
      ...real,
      teamSnapshot: async (org: string, team: string) => {
        const snapshot = await real.teamSnapshot(org, team)
        return team === 'app' ? { ...snapshot, mine: (snapshot.mine ?? []).slice(1) } : snapshot
      },
    }
    renderApp({ node, client, enrolled: true })
    await waitFor(() => expect(counts(native)).toContain(2))
    expect(screen.getByRole('link', { name: /Decisions/ })).toHaveTextContent('2')
    expect(counts(native).every((count) => count === 2)).toBe(true)
  })

  it('goes down when a decision is made', async () => {
    const user = userEvent.setup()
    const native = installApp()
    renderApp()
    await user.click(await screen.findByRole('radio', { name: /Approve/ }))
    await user.click(screen.getByRole('button', { name: /^Approve with/ }))
    await waitFor(() => expect(counts(native).at(-1)).toBe(2), { timeout: 5000 })
  })

  it('says nothing while the owner’s requests are not known: the node cannot be reached', async () => {
    const native = installApp()
    renderApp({ failWith: 500 })
    await screen.findByText(/Could not reach the node/)
    expect(native.setBadge).not.toHaveBeenCalled()
  })

  it('says nothing when one team cannot be read, since the number would be short', async () => {
    const native = installApp()
    const node = createTestNode()
    const client = createApiClient((request) =>
      request.url.includes('/teams/site/view') ? Promise.resolve(new Response(JSON.stringify({ error: 'down' }), { status: 503, headers: { 'content-type': 'application/json' } })) : node.handle(request),
    )
    renderApp({ node, client, enrolled: true })
    await screen.findByText(/Could not reach the node/)
    expect(native.setBadge).not.toHaveBeenCalled()
  })

  it('says nothing once the sign-in expired, and leaves the last number as it was', async () => {
    const native = installApp()
    const node = createTestNode()
    let isExpired = false
    const client = createApiClient(
      // A call that follows Access's redirect fails as a network error; the one that asks without following it gets the redirect.
      (request) => {
        if (!isExpired) return node.handle(request)
        return request.redirect === 'manual' ? Promise.resolve(new Response(null, { status: 302, headers: { location: 'https://login.example.test/' } })) : Promise.reject(new TypeError('Failed to fetch'))
      },
      { hostname: () => 'agency.example.test', isOnline: () => true },
      { confirmAfterMs: 0 },
    )
    renderApp({ node, client, enrolled: true })
    await waitFor(() => expect(counts(native)).toContain(3))
    const before = native.setBadge.mock.calls.length
    isExpired = true
    window.dispatchEvent(new Event('visibilitychange'))
    await screen.findByText('Your sign-in expired. Reload to sign in again.')
    expect(native.setBadge.mock.calls.length).toBe(before)
  })

  it('does nothing in a browser', async () => {
    renderApp()
    expect(await screen.findByRole('option', { name: /Add a second frontend agent/ })).toBeInTheDocument()
  })
})
