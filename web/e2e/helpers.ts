import { expect, type APIRequestContext, type Page } from '@playwright/test'

export interface NodeOptions {
  /** Serve the mock token use (the real node does not yet). */
  usage?: boolean
  /** Make the `site` team a stale copy that could not be refreshed. */
  siteError?: string
  /** What the node reports for an accepted decision. */
  outcome?: 'queued' | 'delivered'
  /** Nothing waits on anyone, and nothing was decided before. */
  empty?: boolean
}

/** Starts the mock node over, with its sample data and the given options. */
export async function resetNode(request: APIRequestContext, options: NodeOptions = {}): Promise<void> {
  const query = new URLSearchParams()
  if (options.usage === true) query.set('usage', '1')
  if (options.siteError !== undefined) query.set('siteError', options.siteError)
  if (options.outcome !== undefined) query.set('outcome', options.outcome)
  if (options.empty === true) query.set('empty', '1')
  const reply = await request.post(`/__mock/reset?${query.toString()}`)
  expect(reply.status()).toBe(204)
}

/** Collects what the page logs as errors, so a test can fail on a broken page that still looks right. */
export function trackErrors(page: Page): () => string[] {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (message) => {
    // A refused request is logged by the browser; the tests that expect one check the screen instead.
    if (message.type() === 'error' && !message.text().includes('Failed to load resource')) errors.push(message.text())
  })
  return () => errors
}

/** True when the page itself scrolls sideways. A chart may scroll inside its own frame, but the page may not. */
export const pageScrollsSideways = (page: Page): Promise<boolean> =>
  page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth)
