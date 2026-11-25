import { screen, waitFor } from '@testing-library/react'
import { NATIVE_PUSH_EVENT } from './push'
import { createApiClient } from '../api/client'
import { createMockNode } from '../mock/node'
import { renderApp } from '../test/render'

const PUSH = { token: 'ab'.repeat(32), environment: 'sandbox', label: 'iPhone' }

const setNative = (push: unknown): void => {
  ;(window as unknown as { agencyNative?: unknown }).agencyNative = { platform: 'ios', push }
}

const announce = (push: unknown): void => {
  setNative(push)
  window.dispatchEvent(new CustomEvent(NATIVE_PUSH_EVENT, { detail: push }))
}

afterEach(() => {
  delete (window as unknown as { agencyNative?: unknown }).agencyNative
})

describe('registering the phone’s push token', () => {
  it('sends the token the app set before the page ran, once', async () => {
    setNative(PUSH)
    const { node } = renderApp()
    await waitFor(() => expect(node.pushTokens()).toEqual([PUSH]))
    await screen.findByRole('article')
    expect(node.pushTokens()).toHaveLength(1)
  })

  it('does nothing outside the app, or before notifications are allowed', async () => {
    const { node } = renderApp()
    await screen.findByRole('article')
    setNative(null)
    window.dispatchEvent(new CustomEvent(NATIVE_PUSH_EVENT, { detail: null }))
    expect(node.pushTokens()).toEqual([])
  })

  it('sends the token when the app announces it while the page is open', async () => {
    const { node } = renderApp()
    await screen.findByRole('article')
    announce(PUSH)
    await waitFor(() => expect(node.pushTokens()).toEqual([PUSH]))
  })

  it('sends a changed token again, and keeps the node’s one entry per token', async () => {
    const { node } = renderApp()
    await screen.findByRole('article')
    announce(PUSH)
    await waitFor(() => expect(node.pushTokens()).toHaveLength(1))
    announce({ ...PUSH, token: 'ff'.repeat(32), environment: 'production' })
    await waitFor(() => expect(node.pushTokens().map((one) => one.token)).toEqual(['ab'.repeat(32), 'ff'.repeat(32)]))
  })

  it('does not send the same token twice from one page', async () => {
    const { node } = renderApp()
    await screen.findByRole('article')
    const spy = vi.spyOn(node, 'handle')
    announce(PUSH)
    await waitFor(() => expect(node.pushTokens()).toHaveLength(1))
    announce(PUSH)
    announce(PUSH)
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(spy.mock.calls.filter(([request]) => request.url.endsWith('/api/push/tokens'))).toHaveLength(1)
  })

  it('refuses a malformed state without calling the node', async () => {
    const { node } = renderApp()
    await screen.findByRole('article')
    announce({ token: 'NOT-HEX', environment: 'sandbox', label: 'iPhone' })
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(node.pushTokens()).toEqual([])
  })

  it('stays quiet when the node does not take tokens yet, and tries again on the next announcement', async () => {
    const { node } = renderApp({ pushTokensFailWith: 404 })
    await screen.findByRole('article')
    announce(PUSH)
    await new Promise((resolve) => setTimeout(resolve, 50))
    expect(node.pushTokens()).toEqual([])
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('sends it again when the announcement comes after a failed call', async () => {
    const node = createMockNode()
    let attempts = 0
    const client = createApiClient((request) => {
      if (!request.url.endsWith('/api/push/tokens')) return node.handle(request)
      attempts += 1
      return attempts === 1 ? Promise.resolve(new Response(JSON.stringify({ error: 'busy' }), { status: 503, headers: { 'content-type': 'application/json' } })) : node.handle(request)
    })
    renderApp({ node, client })
    await screen.findByRole('article')
    announce(PUSH)
    await waitFor(() => expect(attempts).toBe(1))
    expect(node.pushTokens()).toEqual([])
    announce(PUSH)
    await waitFor(() => expect(node.pushTokens()).toEqual([PUSH]))
  })

  it('never writes the token to the console or to storage', async () => {
    const consoles = (['log', 'info', 'warn', 'error', 'debug'] as const).map((level) => vi.spyOn(console, level).mockImplementation(() => undefined))
    try {
      const { node } = renderApp()
      await screen.findByRole('article')
      announce(PUSH)
      await waitFor(() => expect(node.pushTokens()).toHaveLength(1))
      const printed = consoles.flatMap((spy) => spy.mock.calls.flat()).map(String).join(' ')
      expect(printed).not.toContain(PUSH.token)
      expect(JSON.stringify({ ...window.localStorage })).not.toContain(PUSH.token)
      expect(JSON.stringify({ ...window.sessionStorage })).not.toContain(PUSH.token)
    } finally {
      consoles.forEach((spy) => spy.mockRestore())
    }
  })
})
