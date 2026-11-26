import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { enrolBrowser, pageScrollsSideways, resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  await resetNode(request, { rootChanges: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`changes that wait for the root key are an item at the top of the desk, in ${theme} at ${viewport.name} width`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await page.goto(`/desk?theme=${theme}`)
      const notice = page.getByRole('status', { name: 'Changes waiting for the root key' })
      await expect(notice).toBeVisible()
      await expect(notice).toContainText('2 changes wait for the root key’s signature.')
      await expect(notice).toContainText('add member carol (Carol) (Acme, asked')
      await expect(notice).toContainText('run agency root-sign there.')
      expect(await pageScrollsSideways(page)).toBe(false)
      await page.screenshot({ path: `${DIRECTORY}/rootchanges-${theme}-${viewport.name}.png`, animations: 'disabled' })
    })
  }
}

for (const [theme, viewport] of [['light', VIEWPORTS[0]], ['dark', VIEWPORTS[1]]] as const) {
  test(`an approval the root key's node holds is said as it is, in ${theme} at ${viewport.name} width`, async ({ page, request, browser }) => {
    await resetNode(request, { pendingRoot: true })
    await page.setViewportSize({ width: viewport.width, height: viewport.height })
    await enrolBrowser(page, request)
    // A second browser profile is another device, with its own key, and it asks to be added.
    const phoneContext = await browser.newContext({ baseURL: page.url().replace(/\/devices.*$/, '') })
    try {
      const phone = await phoneContext.newPage()
      await phone.goto('/devices')
      await phone.getByRole('button', { name: 'Add this device' }).click()
      await expect(phone.getByText(/Waiting for you to approve this device/)).toBeVisible()
      await page.goto(`/devices?theme=${theme}`)
      await page.getByRole('button', { name: 'The codes match: approve it' }).click()
      const message = page.getByText('Approved here; it takes effect once the root key\'s node signs it: run `agency root-sign` there.')
      await expect(message).toBeVisible()
      await expect(page.getByText('Phone is added.')).toHaveCount(0)
      await expect(page.getByRole('button', { name: 'The codes match: approve it' })).toBeDisabled()
      expect(await pageScrollsSideways(page)).toBe(false)
      await message.scrollIntoViewIfNeeded()
      await page.screenshot({ path: `${DIRECTORY}/rootchanges-pending-${theme}-${viewport.name}.png`, animations: 'disabled' })
    } finally {
      await phoneContext.close()
    }
  })
}
