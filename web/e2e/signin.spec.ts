import { mkdirSync } from 'node:fs'
import { expect, test } from '@playwright/test'
import { decisionsTaken, enrolBrowser, resetNode, waitOnClock } from './helpers'

// A tunnel serves the page under a named host, not the node's own address. `agency.localhost` reaches the
// preview server as one while staying a secure context, which WebCrypto needs; `*.test` is mapped to this
// machine so the "login" origin the expired session redirects to resolves too.
const executablePath = process.env.AGENCY_CHROMIUM_PATH
const PORT = process.env.AGENCY_E2E_PORT ?? '4173'
const PAGE = `http://agency.localhost:${PORT}`

test.use({
  launchOptions: { args: ['--host-resolver-rules=MAP *.test 127.0.0.1'], ...(executablePath === undefined ? {} : { executablePath }) },
})

test.beforeEach(async ({ request }) => {
  await resetNode(request)
})


test('says the sign-in expired when the session ends, keeps the decision, and sends it after a reload', async ({ page, request }) => {
  await enrolBrowser(page, request, PAGE)
  await page.goto(`${PAGE}/desk`)
  const paper = page.getByRole('article')
  await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
  await paper.getByRole('textbox', { name: /^Notes/ }).fill('Start next week.')

  // The access session ends: every call is now redirected to a login on another origin.
  expect((await request.post('/__mock/sign-in?expired=1')).status()).toBe(204)
  await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()

  const banner = page.locator('.signin-notice')
  await expect(banner).toContainText('Your sign-in expired. Reload to sign in again.')
  await expect(page.getByText(/Could not reach the node/)).toHaveCount(0)
  // The decision is kept, not lost: it waits in the tray while the sign-in is gone.
  await expect(page.getByRole('region', { name: 'Sending decisions' })).toBeVisible()

  // The viewer signs in again: the node answers once more, the reload keeps the decision (kept in the browser), the notice is gone, and it is sent.
  expect((await request.post('/__mock/sign-in?expired=0')).status()).toBe(204)
  await banner.getByRole('button', { name: 'Reload' }).click()
  await expect(banner).toHaveCount(0)
  // The mock node's own record says when it took the envelope; the tray empties after that, and not before.
  await decisionsTaken(page, request)
})

test('does not show the sign-in notice, not even for a moment, when the session is out for an instant and holds again', async ({ page, request }) => {
  await enrolBrowser(page, request, PAGE)
  await page.goto(`${PAGE}/desk`)
  const paper = page.getByRole('article')
  await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
  await paper.getByRole('textbox', { name: /^Notes/ }).fill('Start next week.')
  // Told by the page itself, so that a notice that comes up for an instant cannot be missed between two looks.
  await page.evaluate(() => {
    ;(window as unknown as { __noticeSeen: boolean }).__noticeSeen = false
    new MutationObserver(() => {
      if (document.querySelector('.signin-notice') !== null) (window as unknown as { __noticeSeen: boolean }).__noticeSeen = true
    }).observe(document.body, { childList: true, subtree: true })
  })
  expect((await request.post('/__mock/sign-in?expired=1')).status()).toBe(204)
  await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()
  // The call failed with no answer; a moment later the node answers again, before the page asks whether the sign-in holds.
  await page.waitForTimeout(200)
  expect((await request.post('/__mock/sign-in?expired=0')).status()).toBe(204)
  await page.waitForTimeout(2500)
  expect(await page.evaluate(() => (window as unknown as { __noticeSeen: boolean }).__noticeSeen)).toBe(false)
  await expect(page.locator('.signin-notice')).toHaveCount(0)
  // The first try failed with no answer, and the same decision was sent again: the node has it once.
  await decisionsTaken(page, request)
})

test('does not show the sign-in notice when the connection is out for longer than the wait and comes back, while the browser says it is online', async ({ page, request }) => {
  await enrolBrowser(page, request, PAGE)
  await page.goto(`${PAGE}/desk`)
  const paper = page.getByRole('article')
  await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
  await paper.getByRole('textbox', { name: /^Notes/ }).fill('Start next week.')
  await page.evaluate(() => {
    ;(window as unknown as { __noticeSeen: boolean }).__noticeSeen = false
    new MutationObserver(() => {
      if (document.querySelector('.signin-notice') !== null) (window as unknown as { __noticeSeen: boolean }).__noticeSeen = true
    }).observe(document.body, { childList: true, subtree: true })
  })
  // Every call to the node fails as a lost connection does (a network error, no answer, no redirect), for three seconds:
  // longer than the page waits before it asks whether the sign-in holds, and that call fails too.
  await page.route('**/api/**', (route) => route.abort('connectionrefused'))
  await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()
  await page.waitForTimeout(3000)
  await page.unroute('**/api/**')
  await page.waitForTimeout(1500)
  expect(await page.evaluate(() => (window as unknown as { __noticeSeen: boolean }).__noticeSeen)).toBe(false)
  await expect(page.locator('.signin-notice')).toHaveCount(0)
  // The decision waited for the connection and went when it came back: nothing is lost, and nothing says it failed.
  await decisionsTaken(page, request)
})

test('does not call a node that is simply down on this machine an expired sign-in', async ({ page, request }) => {
  await page.goto(`http://127.0.0.1:${PORT}/desk`)
  await expect(page.getByRole('article')).toBeVisible()
  expect((await request.post('/__mock/sign-in?expired=1')).status()).toBe(204)
  // A call that gets no answer is tried again with backoff before it is called failed; the page's clock is moved on by hand.
  await page.clock.install()
  await page.reload()
  // A page on this machine has no sign-in to expire, so this is just a node that cannot be reached.
  await waitOnClock(page, async () => (await page.getByText(/Could not reach the node/).count()) > 0)
  await expect(page.getByText(/Could not reach the node/).first()).toBeVisible()
  await expect(page.locator('.signin-notice')).toHaveCount(0)
})

// What the viewer sees, kept as screenshots next to those of the screens.
const VIEWS = [
  { name: 'desktop', width: 1440, height: 900, theme: 'light' },
  { name: 'desktop', width: 1440, height: 900, theme: 'dark' },
  { name: 'phone', width: 390, height: 844, theme: 'light' },
  { name: 'phone', width: 390, height: 844, theme: 'dark' },
] as const

for (const view of VIEWS) {
  test(`shows the expired sign-in in ${view.theme} at ${view.name} width`, async ({ page, request }) => {
    mkdirSync('e2e-screenshots', { recursive: true })
    await page.setViewportSize({ width: view.width, height: view.height })
    await page.goto(`${PAGE}/desk?theme=${view.theme}`)
    await expect(page.getByRole('article')).toBeVisible()
    expect((await request.post('/__mock/sign-in?expired=1')).status()).toBe(204)
    await page.reload()
    await expect(page.locator('.signin-notice')).toBeVisible()
    await page.screenshot({ path: `e2e-screenshots/signin-expired-${view.theme}-${view.name}.png`, animations: 'disabled' })
  })
}
