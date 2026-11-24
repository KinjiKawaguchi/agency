import { render, type RenderResult } from '@testing-library/react'
import { createMemoryHistory } from '@tanstack/react-router'
import { App } from '../app/App'
import { createApiClient } from '../api/client'
import { createMockNode, type MockNode } from '../mock/node'
import type { TeamView } from '../api/teamview'
import type { Locale } from '../i18n/locale'

interface Options {
  path?: string
  locale?: Locale
  failWith?: number
  view?: TeamView
}

/** Renders the whole app against a fresh mock node. */
export function renderApp(options: Options = {}): RenderResult & { node: MockNode } {
  const node = createMockNode({ ...(options.failWith === undefined ? {} : { failWith: options.failWith }), ...(options.view === undefined ? {} : { view: options.view }) })
  const history = createMemoryHistory({ initialEntries: [options.path ?? '/desk'] })
  const result = render(<App client={createApiClient((request) => node.handle(request))} history={history} locale={options.locale ?? 'en'} />)
  return { ...result, node }
}
