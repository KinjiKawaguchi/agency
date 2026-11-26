import { render, type RenderResult } from '@testing-library/react'
import { createMemoryHistory } from '@tanstack/react-router'
import { App } from '../app/App'
import { createApiClient, type ApiClient } from '../api/client'
import { createQueryClient } from '../app/queryClient'
import { createMockNode, type MockNode } from '../mock/node'
import { createTestNode, testKeyStore } from './testDevice'
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
  evaluation?: boolean
  evaluationFailsWith?: number
  evaluationUnavailableTeam?: string
  activity?: boolean
  activityFailsWith?: number
  activityClock?: () => number
  moreOrgs?: boolean
  usageFailsWith?: number
  refuseDecisionsWith?: string
  earlierDecisions?: boolean
  nothingWaiting?: boolean
  pushTokensFailWith?: number
  decisionLogFailsWith?: number
  /** Use this node instead of a fresh one, for a test that has to act on it first. */
  node?: MockNode
  /** The client to use, to make calls fail in ways the mock node cannot. */
  client?: ApiClient
  /** Where the device key is kept. Unless given, the test device's, which the node already has; or a fresh one when `enrolled` is false. */
  keyStore?: KeyStore
  /** False to render a browser whose device the node does not know yet, as on a first visit. */
  enrolled?: boolean
  outcome?: 'applied' | 'delivered' | 'queued'
  /** The waits between retries of a read that fails in a retryable way; none, by default, so that a test does not sit waiting. */
  retryDelays?: readonly number[]
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
  const isEnrolled = options.enrolled ?? (options.node === undefined && options.keyStore === undefined)
  const create = isEnrolled ? createTestNode : createMockNode
  const node = options.node ?? create({ ...(options.failWith === undefined ? {} : { failWith: options.failWith }), ...(options.view === undefined ? {} : { view: options.view }), ...(options.siteError === undefined ? {} : { siteError: options.siteError }), ...(options.usage === undefined ? {} : { usage: options.usage }), ...(options.evaluation === undefined ? {} : { evaluation: options.evaluation }), ...(options.evaluationFailsWith === undefined ? {} : { evaluationFailsWith: options.evaluationFailsWith }), ...(options.evaluationUnavailableTeam === undefined ? {} : { evaluationUnavailableTeam: options.evaluationUnavailableTeam }), ...(options.activity === undefined ? {} : { activity: options.activity }), ...(options.activityFailsWith === undefined ? {} : { activityFailsWith: options.activityFailsWith }), ...(options.activityClock === undefined ? {} : { activityClock: options.activityClock }), ...(options.moreOrgs === undefined ? {} : { moreOrgs: options.moreOrgs }), ...(options.usageFailsWith === undefined ? {} : { usageFailsWith: options.usageFailsWith }), ...(options.refuseDecisionsWith === undefined ? {} : { refuseDecisionsWith: options.refuseDecisionsWith }), ...(options.outcome === undefined ? {} : { outcome: options.outcome }), ...(options.earlierDecisions === undefined ? {} : { earlierDecisions: options.earlierDecisions }), ...(options.pushTokensFailWith === undefined ? {} : { pushTokensFailWith: options.pushTokensFailWith }), ...(options.nothingWaiting === undefined ? {} : { nothingWaiting: options.nothingWaiting }), ...(options.decisionLogFailsWith === undefined ? {} : { decisionLogFailsWith: options.decisionLogFailsWith }) })
  const history = createMemoryHistory({ initialEntries: [options.path ?? '/desk'] })
  const result = render(<App client={options.client ?? createApiClient((request) => node.handle(request))} history={history} locale={options.locale ?? 'en'} keyStore={options.keyStore ?? (isEnrolled ? testKeyStore() : memoryKeyStore())} queryClient={createQueryClient(options.retryDelays ?? [0, 0])} />)
  return { ...result, node }
}
