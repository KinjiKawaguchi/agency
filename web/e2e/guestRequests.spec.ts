import { expect, test, type Request } from '@playwright/test'
import { decisionsTaken, enrolBrowser, resetNode, trackErrors } from './helpers'

// A member the node hosts (a guest of it) may ask the node for their own decisions and their own devices, and nothing else: every other
// route is the owner's and the node answers it 403 (#201). This follows every request the browser makes while the guest goes through
// the app, and fails on any that is not one of the routes below, whether or not the node would refuse it: a page that quietly starts
// to read an owner's route is found here, not by a guest whose screen shows a refusal.

const ALLOWED: ReadonlyArray<readonly [method: string, path: RegExp]> = [
  ['GET', /^\/api\/me$/],
  ['GET', /^\/api\/decisions$/],
  ['GET', /^\/api\/devices$/],
  ['POST', /^\/api\/devices\/(requests|changes)$/],
  ['GET', /^\/api\/orgs\/[^/]+\/teams\/[^/]+\/view$/],
  ['POST', /^\/api\/orgs\/[^/]+\/teams\/[^/]+\/envelopes$/],
  ['GET', /^\/api\/push\/vapid$/],
  ['GET', /^\/api\/push\/subscriptions$/],
  ['POST', /^\/api\/push\/subscriptions$/],
]

/** Every page of the app that is the owner's, by the path a person could type or a link could lead to. */
const OWNER_PAGES = ['/', '/overview', '/people', '/org', '/members', '/costs', '/evaluation', '/outcomes', '/live'] as const

const isApi = (request: Request): boolean => new URL(request.url()).pathname.startsWith('/api/')

const describe = (request: Request): string => `${request.method()} ${new URL(request.url()).pathname}`

const isAllowed = (request: Request): boolean => {
  const path = new URL(request.url()).pathname
  return ALLOWED.some(([method, pattern]) => method === request.method() && pattern.test(path))
}

let errors: () => string[]
let asked: Request[]
let refused: string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request, { guest: 'carol' })
  errors = trackErrors(page)
  asked = []
  refused = []
  page.on('request', (one) => {
    if (isApi(one)) asked.push(one)
  })
  page.on('response', (one) => {
    if (one.status() === 403) refused.push(describe(one.request()))
  })
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

test('every page of the owner’s leads a guest to the desk, and no request of theirs is the owner’s', async ({ page }) => {
  for (const path of OWNER_PAGES) {
    await page.goto(path)
    await expect(page, `${path} is not opened for a guest`).toHaveURL(/\/desk$/)
    await expect(page.getByText('Add a second frontend agent').first()).toBeVisible()
  }
  expect(asked.length, 'the pages asked the node for something').toBeGreaterThan(0)
  expect(asked.filter((one) => !isAllowed(one)).map(describe)).toEqual([])
  expect(refused).toEqual([])
})

test('a guest decides a request and moves on, and every request is one of their own', async ({ page, request }) => {
  await enrolBrowser(page, request)
  await page.goto('/desk')
  const paper = page.getByRole('article')
  await paper.locator('label.choice').filter({ hasText: 'Approve' }).click()
  await paper.getByRole('button', { name: 'Approve with “Approve”' }).click()
  // The request leaves the pile and the next one opens at once; the kept decision is sent from the tray, with the routes a guest has.
  await expect(page.getByRole('listbox').getByText('Add a second frontend agent')).toHaveCount(0)
  await decisionsTaken(page, request)
  await page.getByRole('navigation', { name: 'Main menu' }).getByRole('link', { name: /^Devices/ }).click()
  await expect(page.getByRole('heading', { level: 1, name: 'Devices' })).toBeVisible()

  expect(asked.some((one) => one.method() === 'POST' && /\/envelopes$/.test(one.url())), 'the decision was sent').toBe(true)
  expect(asked.filter((one) => !isAllowed(one)).map(describe)).toEqual([])
  expect(refused).toEqual([])
})

test('the check fails on an owner route: a read of the usage is not one of a guest’s', async ({ page }) => {
  await page.goto('/desk')
  await expect(page.getByText('Add a second frontend agent').first()).toBeVisible()
  const reply = await page.evaluate(async () => (await fetch('/api/orgs/acme/teams/app/usage')).status)
  expect(reply).toBe(403)
  expect(asked.filter((one) => !isAllowed(one)).map(describe)).toEqual(['GET /api/orgs/acme/teams/app/usage'])
  expect(refused).toEqual(['GET /api/orgs/acme/teams/app/usage'])
  refused = []
  asked = []
})
