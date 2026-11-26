import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { resetNode } from './helpers'

const DIRECTORY = 'e2e-screenshots'

/** The headless browser has refused notifications before anyone is asked; a person's browser has not, so the page is told it can still ask. */
const askable = (): void => {
  Object.defineProperty(Notification, 'permission', { get: () => 'default' })
}

// What the real worker, served by the built client, needs and gives. A real push service is not reachable from the test
// browser, so subscribing itself is covered by the unit tests; here the card, and the worker's notice module, are looked at.

test('serves the notice module the worker imports, as a script, and the worker still takes control', async ({ page, request }) => {
  await resetNode(request)
  const module = await request.get('/push-notice.js')
  expect(module.headers()['content-type']).toMatch(/javascript/)
  expect(await module.text()).toContain('AgencyNotice')
  await page.goto('/overview')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await page.reload()
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)
})

test('draws no switch for a closed app when the node has no Web Push', async ({ page, request }) => {
  await page.addInitScript(askable)
  await resetNode(request)
  await page.goto('/devices')
  await expect(page.getByRole('button', { name: 'Turn on notifications' })).toBeVisible()
  await expect(page.getByText('When the app is closed')).toHaveCount(0)
})

for (const [theme, width, height] of [['light', 1100, 900], ['dark', 390, 844]] as const) {
  test(`draws the switch for a closed app next to the one for a hidden page, in ${theme} at ${width}px`, async ({ page, request }) => {
    await page.addInitScript(askable)
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { webPush: true })
    await page.setViewportSize({ width, height })
    await page.goto(`/devices?theme=${theme}`)
    await expect(page.getByRole('heading', { name: 'When the app is closed' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Turn on for a closed app' })).toBeVisible()
    await page.getByRole('heading', { name: 'Notifications in this browser' }).scrollIntoViewIfNeeded()
    await page.screenshot({ path: `${DIRECTORY}/web-push-card-${theme}-${width}.png`, fullPage: true, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}

// A real push service is not reachable from the test browser, so the browser is made to hold a subscription, which the
// page tells the node (as it does on every open): the node's mock answers with what it has come to know of it.
const holdsASubscription = (key: string): void => {
  Object.defineProperty(Notification, 'permission', { get: () => 'granted' })
  const bytes = Uint8Array.from(atob(key.replace(/-/g, '+').replace(/_/g, '/')), (character) => character.charCodeAt(0))
  const subscription = {
    endpoint: 'https://push.example/abcdefgh',
    options: { applicationServerKey: bytes.buffer },
    toJSON: () => ({ endpoint: 'https://push.example/abcdefgh', keys: { p256dh: 'P', auth: 'A' } }),
    unsubscribe: async () => true,
  }
  PushManager.prototype.getSubscription = async () => subscription as unknown as PushSubscription
}

for (const [theme, width, height] of [['light', 1100, 900], ['dark', 390, 844]] as const) {
  test(`says plainly that a browser's notifications fail, in ${theme} at ${width}px`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { webPush: true, webPushFailing: true })
    await page.addInitScript(holdsASubscription, 'BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8')
    await page.setViewportSize({ width, height })
    await page.goto(`/devices?theme=${theme}`)
    await expect(page.getByText(/did not take the last notice/)).toBeVisible()
    await expect(page.getByText(/push\.example answered 503/)).toBeVisible()
    await expect(page.getByText(/The last notice it took was at/)).toBeVisible()
    await page.getByRole('heading', { name: 'When the app is closed' }).scrollIntoViewIfNeeded()
    await page.screenshot({ path: `${DIRECTORY}/web-push-failing-${theme}-${width}.png`, fullPage: true, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}

