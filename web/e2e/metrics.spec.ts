import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { resetNode } from './helpers'

const DIRECTORY = 'e2e-screenshots'

test('draws nothing of the node’s response time for anyone but the owner', async ({ page, request }) => {
  await resetNode(request, { evaluation: true })
  await page.goto('/evaluation')
  await expect(page.getByRole('heading', { level: 1, name: 'Evaluation' })).toBeVisible()
  await expect(page.getByRole('heading', { level: 2, name: 'App team' })).toBeVisible()
  await expect(page.getByText('Response time of this node')).toHaveCount(0)
})

for (const [variant, theme, width, height] of [['usual', 'light', 1280, 900], ['slow', 'dark', 390, 844]] as const) {
  test(`draws the node’s response time against the budget (${variant}), in ${theme} at ${width}px, with no sideways scroll`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { evaluation: true, metrics: variant })
    await page.setViewportSize({ width, height })
    await page.goto(`/evaluation?theme=${theme}`)
    const heading = page.getByRole('heading', { level: 2, name: 'Response time of this node' })
    await expect(heading).toBeVisible()
    await expect(page.getByText(variant === 'usual' ? /Within the budget/ : /Over the budget/)).toBeVisible()
    await expect(page.getByRole('rowheader', { name: 'Teams', exact: true })).toBeVisible()
    await page.locator('section.mt-section').screenshot({ path: `${DIRECTORY}/metrics-${variant}-${theme}-${width}.png`, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}
