import { mkdirSync } from 'node:fs'
import { expect, test, type APIRequestContext, type Page } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

// Notifications of the operating system, in a real browser: the switch, and a notification for what is new while the
// page is hidden. The browser's own permission prompt cannot be answered from here, and the headless browser reports
// `Notification.permission` as denied even where the context was granted (the Permissions API says granted), so the
// permission, and only it, is answered by the page's script; the notification itself is made by the browser.

let errors: () => string[]

test.beforeEach(async ({ page, request, context }) => {
  await resetNode(request)
  await context.grantPermissions(['notifications'])
  await page.addInitScript(() => {
    Object.defineProperty(Notification, 'permission', { get: () => 'granted', configurable: true })
    Notification.requestPermission = () => Promise.resolve('granted')
  })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

async function askToBeAdded(page: Page, label: string): Promise<void> {
  await page.goto('/devices')
  await page.getByRole('textbox', { name: 'Name for this device' }).fill(label)
  await page.getByRole('button', { name: 'Add this device' }).click()
  await expect(page.getByText(/Waiting for you to approve this device/)).toBeVisible()
}

async function confirmInTerminal(request: APIRequestContext, page: Page): Promise<void> {
  const code = (await page.getByLabel('Pairing code of this device').textContent()) ?? ''
  expect((await request.post(`/__mock/device-add?code=${code}`)).status()).toBe(204)
}

test('turns on from the devices screen, and is still on after a reload', async ({ page }) => {
  await page.goto('/devices')
  await page.getByRole('button', { name: 'Turn on notifications' }).click()
  await expect(page.getByRole('button', { name: 'Turn off notifications' })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('button', { name: 'Turn off notifications' })).toBeVisible()
})

test('shows a notification of the browser for what is new while the page is hidden, and opens it when clicked', async ({ page, request, browser }) => {
  // A page that records what it asks the browser to show, and lets the real notification be made.
  await page.addInitScript(() => {
    const Original = window.Notification
    const made: Array<{ title: string; body: string | undefined }> = []
    const instances: Notification[] = []
    ;(window as unknown as { __made: typeof made; __instances: typeof instances }).__made = made
    ;(window as unknown as { __instances: typeof instances }).__instances = instances
    class Recording extends Original {
      constructor(title: string, options?: NotificationOptions) {
        super(title, options)
        made.push({ title, body: options?.body })
        instances.push(this)
      }
    }
    window.Notification = Recording
  })
  await page.clock.install()
  // Another device is already added, so that this one can approve the one that asks next.
  await askToBeAdded(page, 'Laptop')
  await confirmInTerminal(request, page)
  await page.goto('/overview')
  await expect(page.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible()
  await page.evaluate(() => window.localStorage.setItem('agency.osNotices', 'on'))
  await page.reload()
  await expect(page.getByRole('heading', { level: 1, name: 'Overview' })).toBeVisible()
  await page.evaluate(() => Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true }))
  const other = await browser.newContext({ baseURL: page.url().replace(/\/overview.*$/, '') })
  try {
    await askToBeAdded(await other.newPage(), 'Phone')
    // The page reads again every 30 seconds, hidden or not, once it is on.
    await page.clock.fastForward(35_000)
    await expect.poll(() => page.evaluate(() => (window as unknown as { __made: unknown[] }).__made.length)).toBe(1)
    expect(await page.evaluate(() => (window as unknown as { __made: unknown[] }).__made)).toEqual([{ title: 'Agency', body: 'Phone asks to be added as a device.' }])
    await page.evaluate(() => (window as unknown as { __instances: Notification[] }).__instances[0]?.onclick?.(new Event('click')))
    await expect(page).toHaveURL(/\/devices$/)
  } finally {
    await other.close()
  }
})

for (const view of [
  { name: 'desktop', width: 1440, height: 900, theme: 'light' },
  { name: 'phone', width: 390, height: 844, theme: 'dark' },
] as const) {
  test(`shows the switch, off and on, in ${view.theme} at ${view.name} width`, async ({ page }) => {
    mkdirSync('e2e-screenshots', { recursive: true })
    await page.setViewportSize({ width: view.width, height: view.height })
    await page.goto(`/devices?theme=${view.theme}`)
    const card = page.locator('section[aria-labelledby="os-notices"]')
    await expect(card).toBeVisible()
    await card.screenshot({ path: `e2e-screenshots/os-notices-off-${view.theme}-${view.name}.png`, animations: 'disabled' })
    await page.getByRole('button', { name: 'Turn on notifications' }).click()
    await expect(page.getByRole('button', { name: 'Turn off notifications' })).toBeVisible()
    await card.screenshot({ path: `e2e-screenshots/os-notices-on-${view.theme}-${view.name}.png`, animations: 'disabled' })
    await page.screenshot({ path: `e2e-screenshots/devices-with-notices-${view.theme}-${view.name}.png`, fullPage: true, animations: 'disabled' })
  })
}
