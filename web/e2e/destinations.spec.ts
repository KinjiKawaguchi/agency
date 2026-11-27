import { mkdirSync } from 'node:fs'
import { expect, test, type Page } from '@playwright/test'
import { enrolBrowser, pageScrollsSideways, resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

let errors: () => string[]

test.beforeEach(async ({ request }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  await resetNode(request, { pushDevice: true, webPush: true })
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

/** What an app does when it registers: a token at the node, with the write token the page carries. */
async function registerApp(page: Page, label: string, token: string, device?: string): Promise<void> {
  const status = await page.evaluate(
    async ({ label, token, device }) => {
      const writeToken = document.querySelector('meta[name="agency-token"]')?.getAttribute('content') ?? ''
      const reply = await fetch('/api/push/tokens', {
        method: 'POST',
        headers: { 'content-type': 'application/json', 'x-agency-token': writeToken },
        body: JSON.stringify({ token, environment: 'production', label, ...(device === undefined ? {} : { device }) }),
      })
      return reply.status
    },
    { label, token, device },
  )
  expect(status).toBe(200)
}

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`lists where the node sends notices, and stops one after asking, in ${theme} at ${viewport.name} width`, async ({ page, request }) => {
      errors = trackErrors(page)
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await enrolBrowser(page, request)
      // The phone names this browser's own device, whose id the Devices screen says.
      await page.goto('/devices')
      const idLine = await page.getByText(/Its id is d-[0-9a-f]{12}\./).textContent()
      const device = /d-[0-9a-f]{12}/.exec(idLine ?? '')?.[0]
      expect(device).toBeDefined()
      await registerApp(page, 'Alice’s iPhone', 'ab'.repeat(32), device)
      await registerApp(page, 'Old iPad', 'cd'.repeat(32))
      await page.goto(`/devices?theme=${theme}`)
      const card = page.getByRole('region', { name: 'Where notices are sent' })
      await expect(card.getByText('Tied to Browser.')).toBeVisible()
      await expect(card.getByText('Tied to no device. It stays until you stop it.')).toBeVisible()
      await card.getByRole('button', { name: 'Stop notices to Old iPad' }).click()
      await expect(card.getByRole('group', { name: 'Stop notices to Old iPad?' })).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      await card.scrollIntoViewIfNeeded()
      await card.screenshot({ path: `${DIRECTORY}/destinations-${theme}-${viewport.name}.png`, animations: 'disabled' })
      await card.getByRole('button', { name: 'Stop notices', exact: true }).click()
      await expect(page.getByText('Old iPad no longer gets notices.')).toBeVisible()
      await expect(card.getByText('Old iPad')).toHaveCount(0)
      await expect(card.getByText('Alice’s iPhone')).toBeVisible()
    })
  }
}
