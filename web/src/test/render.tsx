import { render, type RenderResult } from '@testing-library/react'
import { createMemoryHistory } from '@tanstack/react-router'
import { App } from '../app/App'
import { createApiClient, type ApiClient } from '../api/client'
import { createMockNode, type MockNode } from '../mock/node'
import type { TeamView } from '../api/teamview'
import type { KeyStore } from '../device/deviceKey'
import type { Locale } from '../i18n/locale'

interface Options {
  path?: string
  locale?: Locale
  failWith?: number
  view?: TeamView
  siteError?: string
  usage?: boolean
  usageFailsWith?: number
  refuseDecisionsWith?: string
  earlierDecisions?: boolean
  decisionLogFailsWith?: number
  /** Use this node instead of a fresh one, for a test that has to act on it first. */
  node?: MockNode
  /** The client to use, to make calls fail in ways the mock node cannot. */
  client?: ApiClient
  /** Where the device key is kept; a fresh one in memory unless given. */
  keyStore?: KeyStore
  outcome?: 'applied' | 'delivered' | 'queued'
}

/** A device key store that lives only as long as the test. */
export function memoryKeyStore(): KeyStore {
  let kept: CryptoKeyPair | undefined
  return {
    load: () => Promise.resolve(kept),
    save: (pair) => {
      kept = pair
      return Promise.resolve()
    },
  }
}

/** Renders the whole app against a fresh mock node. */
export function renderApp(options: Options = {}): RenderResult & { node: MockNode } {
  const node = options.node ?? createMockNode({ ...(options.failWith === undefined ? {} : { failWith: options.failWith }), ...(options.view === undefined ? {} : { view: options.view }), ...(options.siteError === undefined ? {} : { siteError: options.siteError }), ...(options.usage === undefined ? {} : { usage: options.usage }), ...(options.usageFailsWith === undefined ? {} : { usageFailsWith: options.usageFailsWith }), ...(options.refuseDecisionsWith === undefined ? {} : { refuseDecisionsWith: options.refuseDecisionsWith }), ...(options.outcome === undefined ? {} : { outcome: options.outcome }), ...(options.earlierDecisions === undefined ? {} : { earlierDecisions: options.earlierDecisions }), ...(options.decisionLogFailsWith === undefined ? {} : { decisionLogFailsWith: options.decisionLogFailsWith }) })
  const history = createMemoryHistory({ initialEntries: [options.path ?? '/desk'] })
  const result = render(<App client={options.client ?? createApiClient((request) => node.handle(request))} history={history} locale={options.locale ?? 'en'} keyStore={options.keyStore ?? memoryKeyStore()} />)
  return { ...result, node }
}
