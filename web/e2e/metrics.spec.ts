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

for (const [theme, width, height] of [['light', 1280, 900], ['dark', 375, 812]] as const) {
  test(`draws the hours of the response time, with the budget line and gaps, in ${theme} at ${width}px, with no sideways scroll`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { evaluation: true, metrics: 'usual' })
    await page.setViewportSize({ width, height })
    await page.goto(`/evaluation?theme=${theme}`)
    const heading = page.getByRole('heading', { level: 3, name: 'Hour by hour' })
    await expect(heading).toBeVisible()
    await expect(page.locator('.mt-budget').first()).toContainText('Budget 100 ms')
    await expect(page.getByRole('figure').first()).toBeVisible()
    await page.locator('.mt-trends').first().screenshot({ path: `${DIRECTORY}/metrics-hours-${theme}-${width}.png`, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}


for (const [theme, width, height] of [['light', 1280, 900], ['dark', 1280, 900], ['light', 390, 844], ['dark', 390, 844]] as const) {
  test(`draws what the web client and the iPhone app waited for, apart from the node’s own times, in ${theme} at ${width}px, with no sideways scroll`, async ({ page, request }) => {
    mkdirSync(DIRECTORY, { recursive: true })
    await resetNode(request, { evaluation: true, metrics: 'clients' })
    await page.setViewportSize({ width, height })
    await page.goto(`/evaluation?theme=${theme}`)
    await expect(page.getByRole('heading', { level: 3, name: 'What the web client waited for' })).toBeVisible()
    await expect(page.getByRole('heading', { level: 3, name: 'What the iPhone app waited for' })).toBeVisible()
    // The node's own read of `GET /api/me` is tagged; what a client waited for the same route is not.
    const web = page.locator('.mt-card', { has: page.getByRole('heading', { name: 'What the web client waited for' }) })
    await expect(web.getByText('In-node read')).toHaveCount(0)
    const ios = page.locator('.mt-card', { has: page.getByRole('heading', { name: 'What the iPhone app waited for' }) })
    await web.screenshot({ path: `${DIRECTORY}/metrics-clients-web-${theme}-${width}.png`, animations: 'disabled' })
    await ios.screenshot({ path: `${DIRECTORY}/metrics-clients-ios-${theme}-${width}.png`, animations: 'disabled' })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  })
}
