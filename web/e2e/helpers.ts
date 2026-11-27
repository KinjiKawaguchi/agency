import { expect, test, type APIRequestContext, type Page } from '@playwright/test'

export interface NodeOptions {
  /** Serve the mock token use (the real node does not yet). */
  usage?: boolean
  /** Serve the mock evaluation numbers (the real node does not yet). */
  evaluation?: boolean
  /** Serve the mock activity feed of the live screen (the real node does not yet). */
  activity?: boolean
  /** Make the `site` team a stale copy that could not be refreshed. */
  siteError?: string
  /** What the node reports for an accepted decision. */
  outcome?: 'queued' | 'delivered'
  /** Nothing waits on anyone, and nothing was decided before. */
  empty?: boolean
  /** The node has Web Push: it gives its key and keeps a subscription for each browser. */
  webPush?: boolean
  /** The owner reads the node's own measurements (`usual`, `empty`, or `slow`); `absent` is a node that does not measure. */
  metrics?: 'usual' | 'empty' | 'slow' | 'absent'
  webPushFailing?: boolean
  /** The node has made a claim link for one seat with this code; `claimFails` is how a claim of it ends, when it does not. */
  claim?: { code: string; fails?: 'unknown' | 'expired' | 'other-email' | 'taken' }
  /** The node's configuration names two other home nodes, Work and a Lab on this machine. */
  homeNodes?: boolean
  /** The org's recovery was started and waits, as the node says it. */
  recovery?: boolean
  /** Two changes wait for the root key's signature, as the node tells its owner. */
  rootChanges?: boolean
  /** The root key's node holds a device change for its owner to sign: the approval answers `pending`. */
  pendingRoot?: boolean
  /** A request is addressed to a seat the viewer answers for: the desk says whom it is for. */
  relayedFor?: boolean
  /** The owner is reading: the node serves the outcomes and the readings (the shared samples). Otherwise it answers 403, as to anyone but the owner. */
  outcomes?: boolean
  /** The evaluation carries each team's participants, as it does for the team's owner. */
  participants?: boolean
  /** Adds the orgs Lab and Club: a stuck team, a stale copy, a busy team, and one that cannot be read. */
  moreOrgs?: boolean
  /** The account has a second device, Work laptop, which can be removed. */
  laptop?: boolean
  /** The viewer is this member of the org, hosted by the node as a guest (an id such as `carol`). */
  guest?: string
}

/** Starts the mock node over, with its sample data and the given options. */
export async function resetNode(request: APIRequestContext, options: NodeOptions = {}): Promise<void> {
  const query = new URLSearchParams()
  if (options.usage === true) query.set('usage', '1')
  if (options.evaluation === true) query.set('evaluation', '1')
  if (options.activity === true) query.set('activity', '1')
  if (options.siteError !== undefined) query.set('siteError', options.siteError)
  if (options.outcome !== undefined) query.set('outcome', options.outcome)
  if (options.empty === true) query.set('empty', '1')
  if (options.moreOrgs === true) query.set('orgs', 'more')
  if (options.participants === true) query.set('participants', '1')
  if (options.outcomes === true) query.set('outcomes', '1')
  if (options.webPush === true) query.set('webPush', '1')
  if (options.metrics !== undefined) query.set('metrics', options.metrics)
  if (options.webPushFailing === true) query.set('webPushFailing', '1')
  if (options.homeNodes === true) query.set('homeNodes', '1')
  if (options.recovery === true) query.set('recovery', '1')
  if (options.rootChanges === true) query.set('rootChanges', '1')
  if (options.pendingRoot === true) query.set('pendingRoot', '1')
  if (options.relayedFor === true) query.set('relayedFor', '1')
  if (options.guest !== undefined) query.set('guest', options.guest)
  if (options.laptop === true) query.set('laptop', '1')
  if (options.claim !== undefined) {
    query.set('claim', options.claim.code)
    if (options.claim.fails !== undefined) query.set('claimFails', options.claim.fails)
  }
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

/**
 * Adds this browser as a device, the way the owner does the first time: it asks the node, and the node's
 * terminal confirms the code (`agency device-add`, stood in for by the mock). Decisions need it, since
 * the node takes only decisions signed by a device. `origin` is for a page that is not under the base
 * URL: a browser keeps its key per origin.
 */
export async function enrolBrowser(page: Page, request: APIRequestContext, origin = ''): Promise<string> {
  await page.goto(`${origin}/devices`)
  await page.getByRole('button', { name: 'Add this device' }).click()
  await expect(page.getByText(/Waiting for you to approve this device/)).toBeVisible()
  const code = (await page.getByLabel('Pairing code of this device').textContent()) ?? ''
  expect((await request.post(`/__mock/device-add?code=${code}`)).status()).toBe(204)
  await page.reload()
  await expect(page.getByText(/This device is added\./)).toBeVisible()
  return code
}

/**
 * Elements that stick out past the edge of the screen without a scrolling box of their own around them.
 * `scrollWidth` alone can pass on a phone, where a browser widens its layout to fit wide content and then
 * shows the page zoomed out and swaying sideways, so the boxes are measured too.
 */
export const stickingOut = (page: Page): Promise<string[]> =>
  page.evaluate(() => {
    const edge = document.documentElement.clientWidth
    const clipped = (element: Element): boolean => {
      for (let up = element.parentElement; up !== null && up !== document.body; up = up.parentElement) {
        if (getComputedStyle(up).overflowX !== 'visible') return true
      }
      return false
    }
    const found: string[] = []
    for (const element of Array.from(document.body.querySelectorAll('*'))) {
      const style = getComputedStyle(element)
      if (style.display === 'none' || style.position === 'fixed' || clipped(element)) continue
      const box = element.getBoundingClientRect()
      if (box.width === 0 || box.height === 0) continue
      if (box.right > edge + 0.5 || box.left < -0.5) {
        const text = (element.textContent ?? '').trim().slice(0, 30)
        found.push(`${element.tagName.toLowerCase()}.${String(element.getAttribute('class') ?? '').split(' ')[0]} "${text}" ${Math.round(box.left)}..${Math.round(box.right)} of ${edge}`)
      }
    }
    return found
  })

/**
 * Moves the page's clock on in steps, with the real network answering in between, until `done` holds. A read that
 * is tried again waits on a timer of the page, so a test that installed the fake clock (`page.clock.install()`)
 * needs this to get past the backoff without waiting for it.
 */
export async function waitOnClock(page: Page, done: () => Promise<boolean>): Promise<void> {
  await expect
    .poll(
      async () => {
        await page.clock.runFor(5_000)
        return done()
      },
      { timeout: 30_000, intervals: [100] },
    )
    .toBe(true)
}

/** How many envelopes the mock node has taken: its own record, which does not depend on what the page has drawn yet. */
export async function acceptedEnvelopes(request: APIRequestContext): Promise<number> {
  return ((await (await request.get('/__mock/envelopes')).json()) as unknown[]).length
}

/** Waits for a decision to have been sent and taken: first the mock node's own record of it (never a guess at how long a loaded runner takes), then the tray, which empties after that. */
export async function decisionsTaken(page: Page, request: APIRequestContext, count = 1): Promise<void> {
  // The test's own time is lengthened by what this waits for, so that this wait's message is the one that fails, not the test's clock.
  test.info().setTimeout(test.info().timeout + 60_000)
  await expect.poll(() => acceptedEnvelopes(request), { timeout: 30_000 }).toBe(count)
  await expect(page.getByRole('region', { name: 'Sending decisions' })).toHaveCount(0, { timeout: 30_000 })
}
