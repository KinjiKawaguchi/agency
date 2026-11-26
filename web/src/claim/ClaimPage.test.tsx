import { act, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createApiClient } from '../api/client'
import { deviceKeyOf } from '../device/deviceKey'
import { createMockNode } from '../mock/node'
import { memoryKeyStore, renderApp } from '../test/render'
import { forgetClaimCode } from './claimCode'
import { CLAIM_POLL_MS } from './useClaim'

const CODE = 'k3JfA9xQ2mVb7LzP'
const SEAT = { code: CODE, seat: 'Carol (finance)', org: 'Acme' }

/** The page a person lands on after signing in, with the code the landing page left in this tab. */
function land(claim: Parameters<typeof createMockNode>[0] = { claim: SEAT }, options: { locale?: 'en' | 'ja'; hasCode?: boolean } = {}) {
  if (options.hasCode !== false) window.sessionStorage.setItem('agency.claimCode', CODE)
  const node = createMockNode(claim)
  const requests: Array<{ method: string; url: string; body: string }> = []
  const client = createApiClient(async (request) => {
    requests.push({ method: request.method, url: request.url, body: await request.clone().text() })
    return node.handle(request)
  })
  const keyStore = memoryKeyStore()
  lastKeyStore = keyStore
  const view = renderApp({ path: '/claim/continue', node, client, enrolled: false, keyStore, ...(options.locale === undefined ? {} : { locale: options.locale }) })
  return { node, requests, keyStore, view }
}

let lastKeyStore: ReturnType<typeof memoryKeyStore> | undefined

/** The key pair the last page made, once it has. */
async function keyStoreOf(): Promise<{ held: CryptoKeyPair }> {
  const held = await lastKeyStore?.load()
  if (held === undefined) throw new Error('No key was made.')
  return { held }
}

describe('claiming a seat by link', () => {
  beforeEach(() => {
    forgetClaimCode()
    window.history.replaceState(null, '', '/')
  })

  it('takes the seat with this browser’s key, shows the pairing code large for the owner to compare, and has no bar of the app', async () => {
    const { node, keyStore } = land()
    expect(await screen.findByRole('heading', { level: 1, name: 'Check this code with the person who invited you' })).toBeInTheDocument()
    expect(screen.getByText(/You asked to take “Carol \(finance\)” in Acme/)).toBeInTheDocument()
    const code = screen.getByLabelText('Pairing code of this browser')
    expect(code).toHaveClass('large')
    expect(code).toHaveTextContent(/^[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}$/)
    expect(screen.getByText(/differs in any character, stop/)).toBeInTheDocument()
    // The code is the one of the key this browser made and sent.
    const held = await keyStore.load()
    expect(held).toBeDefined()
    expect(code.textContent).toBe((await deviceKeyOf(held as CryptoKeyPair)).pairingCode)
    expect(node.claim.state()?.pairingCode).toBe(code.textContent)
    expect(screen.queryByRole('navigation', { name: 'Main menu' })).not.toBeInTheDocument()
    // The link is spent: its code is out of this tab, so a reload does not try it again.
    expect(window.sessionStorage.getItem('agency.claimCode')).toBeNull()
  })

  it('goes on by itself once the owner confirms, and says the person is in, with the way to what waits for them', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    try {
      const { node } = land()
      await screen.findByLabelText('Pairing code of this browser')
      node.claim.confirm()
      await act(async () => {
        await vi.advanceTimersByTimeAsync(CLAIM_POLL_MS)
      })
      expect(await screen.findByRole('heading', { level: 1, name: 'You’re in' })).toBeInTheDocument()
      expect(screen.getByText(/You now take “Carol \(finance\)” in Acme/)).toBeInTheDocument()
      expect(screen.getByRole('link', { name: 'Open what waits for you' })).toHaveAttribute('href', '/desk')
      expect(screen.queryByLabelText('Pairing code of this browser')).not.toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })

  it('does not ask whether the owner has confirmed while the tab is hidden', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true })
    try {
      const { node, requests } = land()
      await screen.findByLabelText('Pairing code of this browser')
      const before = requests.length
      node.claim.confirm()
      await act(async () => {
        await vi.advanceTimersByTimeAsync(CLAIM_POLL_MS * 3)
      })
      expect(requests.length).toBe(before)
      expect(screen.queryByRole('heading', { name: 'You’re in' })).not.toBeInTheDocument()
    } finally {
      Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true })
      vi.useRealTimers()
    }
  })

  it('shows where the claim stands when the page is read again with no code: the person has no link to hold any more', async () => {
    const first = land()
    await screen.findByLabelText('Pairing code of this browser')
    first.view.unmount()
    forgetClaimCode()
    // The same key, as the browser keeps it (another store object over it, as a reload makes one).
    renderApp({ path: '/claim/continue', node: first.node, enrolled: false, keyStore: { load: first.keyStore.load, save: first.keyStore.save } })
    expect(await screen.findByLabelText('Pairing code of this browser')).toBeInTheDocument()
  })

  it('says it needs the link when there is no code and no claim to ask about', async () => {
    land({ claim: SEAT }, { hasCode: false })
    expect(await screen.findByText('This page needs the link you were sent. Open that link again.')).toBeInTheDocument()
  })

  it.each([
    [{ ...SEAT, code: 'someOtherCode12345' }, /not valid, or it was already used/],
    [{ ...SEAT, failWith: 'unknown' as const }, /not valid, or it was already used/],
    [{ ...SEAT, failWith: 'expired' as const }, /This link has expired/],
    [{ ...SEAT, failWith: 'taken' as const }, /already taken/],
  ])('says why the link cannot be used (%#), and what to do', async (claim, sentence) => {
    land({ claim })
    expect(await screen.findByRole('alert')).toHaveTextContent(sentence)
    expect(screen.queryByLabelText('Pairing code of this browser')).not.toBeInTheDocument()
    // The code is spent: it is gone from the tab, so a reload does not try it again.
    expect(window.sessionStorage.getItem('agency.claimCode')).toBeNull()
  })

  it('says the link was made for another address, offers to sign out, keeps the code for another try, and goes on once the right address is in', async () => {
    const user = userEvent.setup()
    const { node } = land({ claim: { ...SEAT, failWith: 'other-email' } })
    expect(await screen.findByRole('alert')).toHaveTextContent('made for another address')
    expect(screen.getByRole('link', { name: 'Sign out' })).toHaveAttribute('href', '/cdn-cgi/access/logout')
    expect(window.sessionStorage.getItem('agency.claimCode')).toBe(CODE)
    expect(node.claim.state()).toBeUndefined()
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('made for another address')
  })

  it('says the node cannot be reached, keeps the code, and claims when it is tried again', async () => {
    const user = userEvent.setup()
    const node = createMockNode({ claim: SEAT })
    window.sessionStorage.setItem('agency.claimCode', CODE)
    let isUp = false
    const client = createApiClient((request) => (isUp ? node.handle(request) : Promise.reject(new TypeError('Failed to fetch'))))
    renderApp({ path: '/claim/continue', node, client, enrolled: false, keyStore: memoryKeyStore() })
    expect(await screen.findByRole('alert')).toHaveTextContent('The node cannot be reached')
    expect(window.sessionStorage.getItem('agency.claimCode')).toBe(CODE)
    isUp = true
    await user.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByLabelText('Pairing code of this browser')).toBeInTheDocument()
  })

  it('says this browser cannot make the key, and claims nothing', async () => {
    const spy = vi.spyOn(crypto.subtle, 'generateKey').mockRejectedValue(new Error('not supported'))
    try {
      const { node } = land()
      expect(await screen.findByRole('alert')).toHaveTextContent('cannot make the key your decisions are signed with')
      expect(node.claim.state()).toBeUndefined()
    } finally {
      spy.mockRestore()
    }
  })

  it('sends the code to the node once, in the body of the claim, and puts it in no address, no storage, and no other call', async () => {
    window.history.replaceState(null, '', `/claim/continue#code=${CODE}`)
    const { requests } = land({ claim: SEAT }, { hasCode: false })
    await screen.findByLabelText('Pairing code of this browser')
    expect(window.location.hash).toBe('')
    expect(requests.filter((one) => one.url.includes(CODE))).toEqual([])
    expect(requests.filter((one) => one.body.includes(CODE)).map((one) => `${one.method} ${new URL(one.url).pathname}`)).toEqual(['POST /api/claim'])
    expect(JSON.stringify({ ...window.localStorage })).not.toContain(CODE)
    expect(JSON.stringify({ ...window.sessionStorage })).not.toContain(CODE)
    expect(document.body.textContent).not.toContain(CODE)
  })

  it('refuses a second claim with the same link: it works once', async () => {
    const first = land()
    await screen.findByLabelText('Pairing code of this browser')
    first.view.unmount()
    window.sessionStorage.setItem('agency.claimCode', CODE)
    forgetClaimCode()
    window.sessionStorage.setItem('agency.claimCode', CODE)
    renderApp({ path: '/claim/continue', node: first.node, enrolled: false, keyStore: memoryKeyStore() })
    expect(await screen.findByRole('alert')).toHaveTextContent(/not valid, or it was already used/)
  })

  it('works the pairing code out from this browser’s own key, and shows it, whatever the node answers', async () => {
    land()
    const code = await screen.findByLabelText('Pairing code of this browser')
    const held = await (await import('../device/deviceKey')).deviceKeyOf((await keyStoreOf()).held)
    expect(code).toHaveTextContent(held.pairingCode)
  })

  it('refuses, and shows no code, when the node answers another code than the one of the key this browser made: the owner’s comparison would bind nothing', async () => {
    const { node } = land({ claim: { ...SEAT, answersCode: 'aaaa-bbbb-cccc' } })
    expect(await screen.findByRole('alert')).toHaveTextContent('The node holds another key than the one this browser made')
    expect(screen.queryByLabelText('Pairing code of this browser')).not.toBeInTheDocument()
    expect(document.body.textContent).not.toContain('aaaa-bbbb-cccc')
    expect(screen.queryByRole('button', { name: 'Try again' })).not.toBeInTheDocument()
    expect(window.sessionStorage.getItem('agency.claimCode')).toBeNull()
    expect(node.claim.state()?.state).toBe('waiting')
  })

  it('refuses, and takes the code off the screen, when the node comes to hold another key while the page waits', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    try {
      const { node } = land()
      await screen.findByLabelText('Pairing code of this browser')
      node.claim.holdOtherKey('dddd-eeee-ffff')
      await act(async () => {
        await vi.advanceTimersByTimeAsync(CLAIM_POLL_MS)
      })
      expect(await screen.findByRole('alert')).toHaveTextContent('The node holds another key')
      expect(screen.queryByLabelText('Pairing code of this browser')).not.toBeInTheDocument()
    } finally {
      vi.useRealTimers()
    }
  })

  it('refuses when the page is read again in a browser that no longer has the key it claimed with', async () => {
    const first = land()
    await screen.findByLabelText('Pairing code of this browser')
    first.view.unmount()
    forgetClaimCode()
    // Another store: another key, as after the site data was cleared. The node's answer is about the first.
    renderApp({ path: '/claim/continue', node: first.node, enrolled: false, keyStore: memoryKeyStore() })
    expect(await screen.findByRole('alert')).toHaveTextContent('The node holds another key')
    expect(screen.queryByLabelText('Pairing code of this browser')).not.toBeInTheDocument()
  })

  it('makes the key this browser claims with non-extractable: nothing, not even this page, can read it out', async () => {
    const { keyStore } = land()
    await screen.findByLabelText('Pairing code of this browser')
    const held = (await keyStore.load()) as CryptoKeyPair
    expect(held.privateKey.extractable).toBe(false)
    await expect(crypto.subtle.exportKey('pkcs8', held.privateKey)).rejects.toThrow()
  })

  it('claims once, though the page is drawn again while the claim is on its way', async () => {
    const node = createMockNode({ claim: SEAT })
    window.sessionStorage.setItem('agency.claimCode', CODE)
    const claims: string[] = []
    const client = createApiClient(async (request) => {
      if (request.method === 'POST' && new URL(request.url).pathname === '/api/claim') {
        claims.push(request.url)
        // Slow enough that the page is drawn again (the key's state, the language) before the answer comes.
        await new Promise((resolve) => setTimeout(resolve, 150))
      }
      return node.handle(request)
    })
    renderApp({ path: '/claim/continue', node, client, enrolled: false, keyStore: memoryKeyStore() })
    expect(await screen.findByLabelText('Pairing code of this browser')).toBeInTheDocument()
    expect(claims).toHaveLength(1)
  })

  it('reads in Japanese', async () => {
    land({ claim: SEAT }, { locale: 'ja' })
    expect(await screen.findByRole('heading', { level: 1, name: '招いた人と、このコードを照らし合わせてください' })).toBeInTheDocument()
    expect(screen.getByText(/「Carol \(finance\)」を受け取ろうとしています/)).toBeInTheDocument()
    await waitFor(() => expect(screen.getByLabelText('このブラウザの照合コード')).toBeInTheDocument())
  })
})
