import { expect, test } from '@playwright/test'
import { enrolBrowser, pageScrollsSideways, resetNode, stickingOut, trackErrors } from './helpers'

// What a phone gets: no page that scrolls sideways at the widths of small and large phones, and no
// advice about keys that a touch screen does not have.
const WIDTHS = [375, 430] as const

const ROUTES = [
  { path: '/desk', ready: 'article.paper' },
  { path: '/overview', ready: 'article.ov-team' },
  { path: '/people', ready: 'article.mate' },
  { path: '/org', ready: '.orgchart .oc-node.agent' },
  { path: '/costs', ready: '.columns .column' },
  { path: '/devices', ready: '.pairing-code' },
] as const

test.use({ hasTouch: true })

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request, { usage: true, moreOrgs: true })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

for (const width of WIDTHS) {
  for (const route of ROUTES) {
    test(`${route.path} stays inside the page at ${width}px`, async ({ page }) => {
      await page.setViewportSize({ width, height: 800 })
      await page.goto(route.path)
      await expect(page.locator(route.ready).first()).toBeVisible()
      expect(await pageScrollsSideways(page)).toBe(false)
      expect(await stickingOut(page)).toEqual([])
    })
  }
}

test('a document opened from the pile stays inside the page, with its options and its note', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 })
  await page.goto('/desk')
  await page.getByRole('option', { name: /Which database should the app use in production/ }).click()
  await page.locator('label.choice').first().click()
  await page.getByRole('textbox').first().fill('x'.repeat(200))
  expect(await pageScrollsSideways(page)).toBe(false)
  expect(await stickingOut(page)).toEqual([])
})

test('the desk does not tell a touch screen about keys', async ({ page, request }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await enrolBrowser(page, request)
  await page.goto('/desk')
  await expect(page.getByRole('button', { name: 'Approve' })).toBeVisible()
  expect(await page.evaluate(() => matchMedia('(hover: none) and (pointer: coarse)').matches)).toBe(true)
  await expect(page.locator('.hint')).toHaveCount(1)
  await expect(page.locator('.hint')).toBeHidden()
  await expect(page.locator('.kbd').first()).toBeHidden()
})

test.describe('with a keyboard', () => {
  test.use({ hasTouch: false })

  test('the desk still advises the keys', async ({ page, request }) => {
    await enrolBrowser(page, request)
    await page.goto('/desk')
    await expect(page.locator('.hint')).toBeVisible()
    await expect(page.getByText('number keys pick an option')).toBeVisible()
  })
})

test('a screen of controls does not make a phone zoom in when one is tapped', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  for (const path of ['/desk', '/devices', '/people', '/costs', '/org']) {
    await page.goto(path)
    const sizes = await page.locator('input, select, textarea').evaluateAll((all) => all.map((one) => (parseFloat(getComputedStyle(one).fontSize) < 16 ? one.outerHTML.slice(0, 80) : '')).filter(Boolean))
    expect(sizes, path).toEqual([])
  }
})

for (const route of ROUTES) {
  test(`${route.path} holds a long id or path that cannot break, at 375px`, async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 800 })
    await page.goto(route.path)
    await expect(page.locator(route.ready).first()).toBeVisible()
    // Every line of text gets a word with no place to break, as a path or an id has. A time, a status
    // word, and a percentage are short values we format ourselves, so they are left as they are.
    await page.evaluate(() => {
      for (const element of Array.from(document.querySelectorAll('main *, .appbar *'))) {
        if (element.children.length === 0 && (element.textContent ?? '').trim() !== '' && element.closest('select, option, button, table, .stamp-time, .decided-tag, .fund-pct, .fund-left') === null) {
          element.textContent = `${element.textContent} ${'x'.repeat(70)}`
        }
      }
    })
    expect(await stickingOut(page)).toEqual([])
    expect(await pageScrollsSideways(page)).toBe(false)
  })
}
