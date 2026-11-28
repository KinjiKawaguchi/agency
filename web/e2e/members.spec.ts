import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, stickingOut, trackErrors } from './helpers'

const DIRECTORY = 'e2e-screenshots'
const VIEWPORTS = [
  { name: 'desktop', width: 1280, height: 900 },
  { name: 'phone', width: 390, height: 844 },
] as const

let errors: () => string[]

test.beforeEach(({ page }) => {
  mkdirSync(DIRECTORY, { recursive: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const theme of ['light', 'dark'] as const) {
  for (const viewport of VIEWPORTS) {
    test(`shows the owner an org’s members, seats and what waits for the root key, in ${theme} at ${viewport.name} width`, async ({ page, request }) => {
      await resetNode(request, { members: true })
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
      await page.goto(`/members?theme=${theme}`)
      await expect(page.getByRole('heading', { level: 1, name: 'Members' })).toBeVisible()
      await expect(page.getByText('Hosted by this node · A sign-in is set')).toBeVisible()
      await expect(page.getByText(/^remove device d-2ad2694ace04 of carol, received /)).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      expect(await stickingOut(page)).toEqual([])
      await page.screenshot({ path: `${DIRECTORY}/members-${theme}-${viewport.name}.png`, fullPage: true, animations: 'disabled' })
    })
  }
}

test('has no entry in the menu and says so on a node that does not report members', async ({ page, request }) => {
  await resetNode(request, {})
  await page.goto('/members')
  await expect(page.getByText('This node does not report members yet.')).toBeVisible()
  await expect(page.getByRole('link', { name: 'Members' })).toHaveCount(0)
})
