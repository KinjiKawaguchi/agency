import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  await resetNode(request, { recovery: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`a recovery that waits is at the top of the desk, in ${theme} at ${viewport.name} width`, async ({ page }) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await page.goto(`/desk?theme=${theme}`)
      const notice = page.getByRole('alert', { name: 'Recovery of an org' })
      await expect(notice).toBeVisible()
      await expect(notice).toContainText('A recovery of Acme was started. It takes effect at')
      await expect(notice).toContainText('agency root-cancel-recovery --org acme')
      await expect(notice.getByRole('button')).toHaveCount(0)
      expect(await pageScrollsSideways(page)).toBe(false)
      await page.screenshot({ path: `${DIRECTORY}/recovery-${theme}-${viewport.name}.png`, animations: 'disabled' })
    })
  }
}
