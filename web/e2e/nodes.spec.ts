import { expect, test } from '@playwright/test'
import { pageScrollsSideways, resetNode, stickingOut, trackErrors } from './helpers'

// Other home nodes are links. The page reads from its own origin only, never from another home node's.
const OTHERS = JSON.stringify({
  self: 'Personal',
  others: [
    { label: 'Work', origin: 'https://work.example.com' },
    { label: 'A home node with a very long name that must not widen the page', origin: 'https://a-very-long-host-name-for-a-home-node.example.com:8443' },
  ],
})

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request, { usage: true })
  await page.addInitScript((stored) => window.localStorage.setItem('agency.homeNodes', stored), OTHERS)
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const width of [375, 430]) {
  test(`the node's name and the list stay inside the page at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 })
    await page.goto('/devices')
    await expect(page.locator('.node-chip')).toHaveText('Personal')
    await expect(page.getByRole('region', { name: 'Home nodes' }).getByRole('link', { name: 'Work' })).toBeVisible()
    expect(await pageScrollsSideways(page)).toBe(false)
    expect(await stickingOut(page)).toEqual([])
    const menus = await page.locator('.mainnav').evaluateAll((all) => all.map((one) => one.scrollWidth <= one.clientWidth))
    expect(menus.every(Boolean)).toBe(true)
  })
}

test('every request of every screen goes to this origin, and a listed node is only opened by a click', async ({ page, baseURL }) => {
  const origin = new URL(baseURL ?? 'http://127.0.0.1:4173').origin
  const others: string[] = []
  page.on('request', (request) => {
    if (new URL(request.url()).origin !== origin) others.push(request.url())
  })
  // The listed node is not there; answer for it so the click can be seen to land.
  await page.route('https://work.example.com/**', (route) => route.fulfill({ status: 200, contentType: 'text/html', body: '<title>work</title>' }))
  for (const path of ['/desk', '/overview', '/org', '/people', '/costs', '/devices']) {
    await page.goto(path)
    await page.waitForLoadState('networkidle')
  }
  expect(others).toEqual([])
  await page.getByRole('region', { name: 'Home nodes' }).getByRole('link', { name: 'Work' }).click()
  await expect(page).toHaveURL('https://work.example.com/')
})

test('adds a node from the form, and the name chip follows', async ({ page }) => {
  await page.goto('/devices')
  const card = page.getByRole('region', { name: 'Home nodes' })
  await card.getByLabel('Name', { exact: true }).fill('Lab')
  await card.getByLabel('Address').fill('lab.example.org')
  await card.getByRole('button', { name: 'Add this home node' }).click()
  await expect(card.getByRole('link', { name: 'Lab' })).toHaveAttribute('href', 'https://lab.example.org')
  await card.getByRole('button', { name: 'Remove Lab from the list' }).click()
  await expect(card.getByRole('link', { name: 'Lab' })).toHaveCount(0)
})
