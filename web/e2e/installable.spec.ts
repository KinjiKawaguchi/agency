import { expect, test, type Page } from '@playwright/test'
import { resetNode } from './helpers'

// What a browser needs to offer the app for install and to open it as an app, asked of the browser itself (its own
// installability check and its parse of the manifest, through the DevTools protocol), not of our reading of the files.

test.beforeEach(async ({ request }) => {
  await resetNode(request, {})
})

/** Loads the page until the worker controls it: a browser offers an install only to a page its worker has taken. */
async function controlledPage(page: Page): Promise<void> {
  await page.goto('/overview')
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready
  })
  await page.reload()
  await expect.poll(() => page.evaluate(() => navigator.serviceWorker.controller !== null)).toBe(true)
}

const pngSizeOf = (bytes: Buffer): string => `${bytes.readUInt32BE(16)}x${bytes.readUInt32BE(20)}`

test('the browser finds nothing in the way of installing the app, and reads the manifest without a complaint', async ({ page, context }) => {
  await controlledPage(page)
  const session = await context.newCDPSession(page)
  const { installabilityErrors } = (await session.send('Page.getInstallabilityErrors')) as { installabilityErrors: Array<{ errorId: string }> }
  expect(installabilityErrors.map((one) => one.errorId)).toEqual([])
  const { errors, data, url } = (await session.send('Page.getAppManifest')) as { errors: Array<{ message: string }>; data: string; url: string }
  expect(errors.map((one) => one.message)).toEqual([])
  expect(url).toMatch(/\/manifest\.webmanifest$/)
  const manifest = JSON.parse(data) as { id: string; start_url: string; scope: string; display: string }
  expect(manifest.display).toBe('standalone')
  // The app opens on a page inside its scope, with nothing in the address that would have to stay secret or stay the same.
  const start = new URL(manifest.start_url, url)
  expect(start.pathname.startsWith(new URL(manifest.scope, url).pathname)).toBe(true)
  expect(start.search + start.hash).toBe('')
  expect(new URL(manifest.id, url).pathname).toBe(start.pathname)
})

test('each icon is the size the manifest says, as a picture', async ({ request }) => {
  const manifest = (await (await request.get('/manifest.webmanifest')).json()) as { icons: Array<{ src: string; sizes: string }> }
  expect(manifest.icons.map((icon) => icon.sizes)).toEqual(expect.arrayContaining(['192x192', '512x512']))
  for (const icon of manifest.icons) {
    expect(pngSizeOf(await (await request.get(icon.src)).body()), icon.src).toBe(icon.sizes)
  }
})
