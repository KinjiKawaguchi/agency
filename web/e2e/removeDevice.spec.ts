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
  await resetNode(request, { laptop: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`asks before removing a device, names it, and removes it once confirmed, in ${theme} at ${viewport.name} width`, async ({ page, request }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await enrolBrowser(page, request)
      await page.goto(`/devices?theme=${theme}`)
      await page.getByRole('button', { name: 'Remove Work laptop' }).click()
      const panel = page.getByRole('group', { name: 'Remove Work laptop?' })
      await expect(panel.getByText('Work laptop (pairing code dbc2-9825-1c51) can never sign again.')).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      await page.screenshot({ path: `${DIRECTORY}/remove-device-${theme}-${viewport.name}.png`, animations: 'disabled' })
      await panel.getByRole('button', { name: 'Remove this device' }).click()
      await expect(page.getByText('Work laptop is removed. It can never sign again.')).toBeVisible()
      await expect(page.getByRole('button', { name: 'Remove Work laptop' })).toHaveCount(0)
      const listed = (await (await request.get('/api/devices')).json()) as { orgs: Array<{ devices: Array<{ label: string }> }> }
      expect(listed.orgs.flatMap((org) => org.devices.map((device) => device.label))).not.toContain('Work laptop')
    })
  }
}
