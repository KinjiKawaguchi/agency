import { expect, test } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

const PUSH = { token: 'ab'.repeat(32), environment: 'sandbox', label: 'iPhone' }

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request)
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

const registered = async (request: import('@playwright/test').APIRequestContext) => (await (await request.get('/__mock/push-tokens')).json()) as Array<{ token: string }>

test('inside the app: registers the token the app set before the page ran, and the one it announces later', async ({ page, request }) => {
  // What the app's script does at document start.
  await page.addInitScript((push) => {
    ;(window as unknown as { agencyNative: unknown }).agencyNative = { platform: 'ios', push }
  }, PUSH)
  await page.goto('/desk')
  await expect(page.getByRole('article')).toBeVisible()
  await expect.poll(async () => (await registered(request)).map((one) => one.token)).toEqual([PUSH.token])

  // The token changes while the page is open: the app sets it again, then announces it.
  await page.evaluate(() => {
    const push = { token: 'ff'.repeat(32), environment: 'production', label: 'iPhone' }
    ;(window as unknown as { agencyNative: unknown }).agencyNative = { platform: 'ios', push }
    window.dispatchEvent(new CustomEvent('agency:native-push', { detail: push }))
  })
  await expect.poll(async () => (await registered(request)).map((one) => one.token)).toEqual([PUSH.token, 'ff'.repeat(32)])
})

test('outside the app: sends nothing, and the page works as before', async ({ page, request }) => {
  await page.goto('/desk')
  await expect(page.getByRole('article')).toBeVisible()
  await page.waitForTimeout(500)
  expect(await registered(request)).toEqual([])
})
