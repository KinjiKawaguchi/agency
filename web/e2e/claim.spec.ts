import { mkdirSync } from 'node:fs'
import { expect, test, type Page, type Request } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

// A seat claimed by link, in a real browser: the public landing page takes the code out of the address and keeps it for the
// tab, the page behind the sign-in claims the seat, the pairing code waits for the owner, and the person is in. The code is
// in no address after the first, and in no request but the claim's body. The sign-in itself is Access's, which the mock
// node does not have: the step between the two pages is a plain link here.
const CODE = 'k3JfA9xQ2mVb7LzP'

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request, { claim: { code: CODE } })
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

const confirmAtTheNode = async (request: Parameters<typeof resetNode>[0]): Promise<void> => {
  expect((await request.post('/__mock/claim-confirm')).status()).toBe(204)
}

test('lands on the public page, goes on behind the sign-in, shows the pairing code, and is in once the owner confirms; the code goes only where it must', async ({ page, request }) => {
  const seen: Array<{ method: string; url: string; body: string }> = []
  page.on('request', (one: Request) => seen.push({ method: one.method(), url: one.url(), body: one.postData() ?? '' }))

  await page.goto(`/claim.html#code=${CODE}`)
  await expect(page.getByRole('link', { name: 'Sign in to continue' })).toBeVisible()
  // The code is out of the address as soon as the page has read it, and the page holds nothing else of it on show.
  expect(page.url()).not.toContain('#')
  expect(page.url()).not.toContain(CODE)
  expect(await page.evaluate(() => document.body.textContent)).not.toContain(CODE)
  expect(await page.evaluate(() => window.sessionStorage.getItem('agency.claimCode'))).toBe(CODE)

  await page.getByRole('link', { name: 'Sign in to continue' }).click()
  await expect(page).toHaveURL(/\/claim\/continue$/)
  await expect(page.getByRole('heading', { level: 1, name: 'Check this code with the person who invited you' })).toBeVisible()
  await expect(page.getByLabel('Pairing code of this browser')).toHaveText(/^[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}$/)
  await expect(page.getByRole('navigation', { name: 'Main menu' })).toHaveCount(0)
  // Spent: out of this tab's storage, and the page can be reloaded without it.
  expect(await page.evaluate(() => window.sessionStorage.getItem('agency.claimCode'))).toBeNull()

  await confirmAtTheNode(request)
  await expect(page.getByRole('heading', { level: 1, name: 'You’re in' })).toBeVisible({ timeout: 10_000 })

  // Every address of every request is free of the code, and one call, the claim, carries it in its body.
  expect(seen.filter((one) => one.url.includes(CODE))).toEqual([])
  expect(seen.filter((one) => one.body.includes(CODE)).map((one) => `${one.method} ${new URL(one.url).pathname}`)).toEqual(['POST /api/claim'])
  // The landing page reads nothing from the node: only its own page, its script, and its style.
  const beforeSignIn = seen.slice(0, seen.findIndex((one) => one.url.includes('/claim/continue')))
  expect(beforeSignIn.map((one) => new URL(one.url).pathname).sort()).toEqual(['/claim.html', '/landing/claim.css', '/landing/claim.js'])
})

test('is read again after a reload, with no code: the claim is asked about, not claimed again', async ({ page }) => {
  await page.goto(`/claim.html#code=${CODE}`)
  await page.getByRole('link', { name: 'Sign in to continue' }).click()
  await expect(page.getByLabel('Pairing code of this browser')).toBeVisible()
  await page.reload()
  await expect(page.getByLabel('Pairing code of this browser')).toBeVisible()
})

test('says the link is spent when it is opened a second time', async ({ page, context }) => {
  await page.goto(`/claim.html#code=${CODE}`)
  await page.getByRole('link', { name: 'Sign in to continue' }).click()
  await expect(page.getByLabel('Pairing code of this browser')).toBeVisible()
  const other = await context.newPage()
  await other.goto(`/claim.html#code=${CODE}`)
  await other.getByRole('link', { name: 'Sign in to continue' }).click()
  await expect(other.getByRole('alert')).toContainText('not valid, or it was already used')
})

test('says it needs the link, and offers no sign-in, when the landing page is opened with none', async ({ page }) => {
  await page.goto('/claim.html')
  await expect(page.getByText('This page needs the link you were sent. Open that link again.')).toBeVisible()
  await expect(page.getByRole('link', { name: 'Sign in to continue' })).toHaveCount(0)
})

test('is stood in for by the page of the app where the node does not serve the landing page, reading the fragment itself', async ({ page }) => {
  await page.goto(`/claim#code=${CODE}`)
  await expect(page.getByLabel('Pairing code of this browser')).toBeVisible()
  expect(page.url()).not.toContain('#')
})

// The steps, kept as screenshots next to those of the screens.
const VIEWS = [
  { name: 'desktop', width: 1440, height: 900, theme: 'light' },
  { name: 'phone', width: 390, height: 844, theme: 'dark' },
] as const

async function shoot(page: Page, state: string, view: (typeof VIEWS)[number]): Promise<void> {
  mkdirSync('e2e-screenshots', { recursive: true })
  await page.screenshot({ path: `e2e-screenshots/claim-${state}-${view.theme}-${view.name}.png`, fullPage: true, animations: 'disabled' })
}

for (const view of VIEWS) {
  test(`shows the steps in ${view.theme} at ${view.name} width`, async ({ page, request }) => {
    await page.emulateMedia({ colorScheme: view.theme })
    await page.setViewportSize({ width: view.width, height: view.height })
    await page.goto(`/claim.html#code=${CODE}`)
    await expect(page.getByRole('link', { name: 'Sign in to continue' })).toBeVisible()
    await shoot(page, 'landing', view)
    await page.getByRole('link', { name: 'Sign in to continue' }).click()
    await expect(page.getByLabel('Pairing code of this browser')).toBeVisible()
    await shoot(page, 'waiting', view)
    await confirmAtTheNode(request)
    await expect(page.getByRole('heading', { name: 'You’re in' })).toBeVisible({ timeout: 10_000 })
    await shoot(page, 'in', view)
    await resetNode(request, { claim: { code: CODE, fails: 'other-email' } })
    await page.evaluate((code) => window.sessionStorage.setItem('agency.claimCode', code), CODE)
    await page.goto('/claim/continue')
    await expect(page.getByRole('alert')).toContainText('made for another address')
    await shoot(page, 'other-address', view)
  })
}
