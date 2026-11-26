import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { resetNode } from './helpers'

// The installed-app pieces, in a real browser against the built client under the node's security headers: the
// manifest and the worker are served as what they are, the worker takes control, and it keeps built files and nothing else.

test.beforeEach(async ({ request }) => {
  await resetNode(request, {})
})

test('serves the manifest and the icons it names, as what they are', async ({ page, request }) => {
  const manifest = await request.get('/manifest.webmanifest')
  expect(manifest.status()).toBe(200)
  expect(manifest.headers()['content-type']).toMatch(/json/)
  const body = (await manifest.json()) as { icons: Array<{ src: string }>; start_url: string }
  expect(body.start_url).toBe('/desk')
  for (const icon of body.icons) {
    const reply = await request.get(icon.src)
    expect(reply.status()).toBe(200)
    expect(reply.headers()['content-type']).toMatch(/image\/png/)
  }
  await page.goto('/desk')
  await expect(page.locator('link[rel="manifest"]')).toHaveAttribute('crossorigin', 'use-credentials')
})

test('registers the worker, which takes control and keeps built files only', async ({ page }) => {
  await page.goto('/overview')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await page.reload()
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)
  await page.getByRole('link', { name: 'Overview' }).first().waitFor()
  const kept = await page.evaluate(async () => {
    const urls: string[] = []
    for (const name of await caches.keys()) for (const request of await (await caches.open(name)).keys()) urls.push(new URL(request.url).pathname)
    return urls
  })
  expect(kept.some((path) => path.startsWith('/assets/'))).toBe(true)
  // The page and the API are never kept: the pages carry the write token, and the API holds the org's data.
  expect(kept.filter((path) => !path.startsWith('/assets/') && !path.startsWith('/icons/') && path !== '/offline.html' && path !== '/offline.css')).toEqual([])
})

test('does not keep the page the node answers for a built file it does not have', async ({ page }) => {
  await page.goto('/overview')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await page.reload()
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)
  // The node answers any path it does not know with the page: asking for a built file that is gone gets a 200 page.
  const answer = await page.evaluate(async () => {
    const first = await fetch('/assets/gone-OLDHASH.js')
    await fetch('/icons/gone.png')
    return { type: first.headers.get('content-type') }
  })
  expect(answer.type).toMatch(/html/)
  const kept = await page.evaluate(async () => {
    const urls: string[] = []
    for (const name of await caches.keys()) for (const request of await (await caches.open(name)).keys()) urls.push(new URL(request.url).pathname)
    return urls
  })
  expect(kept.filter((path) => path.includes('gone'))).toEqual([])
})

test('shows an offline page, and never a page it kept, when the node cannot be reached', async ({ page, context }) => {
  await page.goto('/overview')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await page.reload()
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)
  await context.setOffline(true)
  await page.goto('/overview').catch(() => undefined)
  await expect(page.getByRole('heading', { name: 'The node cannot be reached' })).toBeVisible()
  await context.setOffline(false)
  await page.goto('/overview')
  await expect(page.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible()
})

test('forgets the built files it kept when it is told the viewer signed out', async ({ page, request }) => {
  await page.goto('/overview')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await page.reload()
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)
  expect(await page.evaluate(async () => (await caches.keys()).includes('agency-assets-1'))).toBe(true)
  // Told that the viewer signed out, the worker deletes them.
  await page.evaluate(() => navigator.serviceWorker.controller?.postMessage({ type: 'signed-out' }))
  await expect.poll(() => page.evaluate(async () => (await caches.keys()).includes('agency-assets-1'))).toBe(false)
  expect(request).toBeDefined()
})

for (const theme of ['light', 'dark'] as const) {
  test(`offers to install the app on the Devices screen when the browser does, in ${theme}`, async ({ page }) => {
    mkdirSync('e2e-screenshots', { recursive: true })
    await page.setViewportSize({ width: 1100, height: 760 })
    await page.goto(`/devices?theme=${theme}`)
    await expect(page.getByRole('heading', { level: 1, name: 'Devices' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Install Agency' })).toHaveCount(0)
    // What a Chromium browser fires when the app can be installed.
    await page.evaluate(() => {
      const event = Object.assign(new Event('beforeinstallprompt', { cancelable: true }), { prompt: () => Promise.resolve(), userChoice: Promise.resolve({ outcome: 'dismissed' }) })
      window.dispatchEvent(event)
    })
    await expect(page.getByRole('button', { name: 'Install Agency' })).toBeVisible()
    await page.screenshot({ path: `e2e-screenshots/install-card-${theme}.png`, fullPage: true, animations: 'disabled' })
    await page.getByRole('button', { name: 'Install Agency' }).click()
    await expect(page.getByRole('button', { name: 'Install Agency' })).toHaveCount(0)
  })
}
