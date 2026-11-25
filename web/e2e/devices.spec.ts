import { mkdirSync } from 'node:fs'
import { expect, test, type APIRequestContext, type Page } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

const CODE = /^[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}$/

let errors: () => string[]

test.beforeEach(async ({ page, request }) => {
  await resetNode(request)
  errors = trackErrors(page)
})

test.afterEach(() => {
  expect(errors()).toEqual([])
})

test('makes a key on first use, keeps the same one across reloads, and keeps it from being read out', async ({ page }) => {
  await page.goto('/desk')
  await page.getByRole('link', { name: 'Devices' }).click()
  const code = page.getByLabel('Pairing code of this device')
  await expect(code).toHaveText(CODE)
  const first = await code.textContent()

  await page.reload()
  await expect(code).toHaveText(first ?? '')

  // What the browser stored is a key pair whose private half cannot be exported.
  const exportable = await page.evaluate(async () => {
    const database = await new Promise<IDBDatabase>((resolve, reject) => {
      const opening = indexedDB.open('agency-device', 1)
      opening.onsuccess = () => resolve(opening.result)
      opening.onerror = () => reject(opening.error)
    })
    const pair = await new Promise<CryptoKeyPair>((resolve, reject) => {
      const reading = database.transaction('keys').objectStore('keys').get('device')
      reading.onsuccess = () => resolve(reading.result as CryptoKeyPair)
      reading.onerror = () => reject(reading.error)
    })
    const canExport = await crypto.subtle.exportKey('pkcs8', pair.privateKey).then(
      () => true,
      () => false,
    )
    return { extractable: pair.privateKey.extractable, canExport }
  })
  expect(exportable).toEqual({ extractable: false, canExport: false })
})

/** What `agency device-add` does on the node's terminal, after the person compared the codes. */
async function confirmInTerminal(request: APIRequestContext, code: string): Promise<void> {
  expect((await request.post(`/__mock/device-add?code=${code}`)).status()).toBe(204)
}

async function askToBeAdded(page: Page, label: string): Promise<string> {
  await page.goto('/devices')
  await page.getByRole('textbox', { name: 'Name for this device' }).fill(label)
  await page.getByRole('button', { name: 'Add this device' }).click()
  await expect(page.getByText(/Waiting for you to confirm this device/)).toBeVisible()
  return (await page.getByLabel('Pairing code of this device').textContent()) ?? ''
}

test('asks to be added, is confirmed in the terminal, then signs a decision the node verifies', async ({ page, request }) => {
  const code = await askToBeAdded(page, 'Work laptop')
  expect(code).toMatch(CODE)
  await confirmInTerminal(request, code)
  // It asks again every few seconds while it waits.
  await expect(page.getByText(/This device is added\./)).toBeVisible({ timeout: 10_000 })
  await expect(page.getByText(`Its id is d-${code.replaceAll('-', '')}.`)).toBeVisible()

  await page.getByRole('link', { name: 'Decisions' }).click()
  await page.getByRole('article').locator('label.choice').filter({ hasText: 'Approve' }).click()
  await page.getByRole('button', { name: 'Approve with “Approve”' }).click()
  await expect(page.getByText('Sent. Lead will get it.')).toBeVisible()

  // The mock node checked the signature of the browser's WebCrypto key over the canonical text.
  const accepted = (await (await request.get('/__mock/envelopes')).json()) as Array<{ signer: string; body: { target: string; kind: string } }>
  expect(accepted).toMatchObject([{ signer: `d-${code.replaceAll('-', '')}`, body: { kind: 'answer', target: 'r-20261124-113000-c3d4' } }])
  await expect(page.getByRole('region', { name: /Recently decided/ }).getByRole('listitem').first()).toContainText('Chose “Approve”.')
})

test('confirms a second device from the first, by comparing codes', async ({ page, request, browser }) => {
  await confirmInTerminal(request, await askToBeAdded(page, 'Laptop'))
  await page.reload()
  await expect(page.getByText(/This device is added\./)).toBeVisible()

  // A second browser profile is another device: its own IndexedDB, its own key.
  const phoneContext = await browser.newContext({ baseURL: page.url().replace(/\/devices.*$/, '') })
  try {
    const phone = await phoneContext.newPage()
    const phoneCode = await askToBeAdded(phone, 'Phone')
    expect(phoneCode).not.toBe(await page.getByLabel('Pairing code of this device').textContent())

    await page.reload()
    await expect(page.getByLabel('Pairing code of Phone')).toHaveText(phoneCode)
    await page.getByRole('button', { name: 'The codes match: add it' }).click()
    await expect(page.getByText('Phone is added.')).toBeVisible()
    await expect(page.getByLabel('Pairing code of Phone')).toHaveCount(0)

    await expect(phone.getByText(/This device is added\./)).toBeVisible({ timeout: 10_000 })
    await expect(phone.getByRole('list').getByText('Laptop')).toBeVisible()
  } finally {
    await phoneContext.close()
  }
})

// The states of the flow, kept as screenshots next to those of the screens.
const VIEWS = [
  { name: 'desktop', width: 1440, height: 900, theme: 'light' },
  { name: 'phone', width: 390, height: 844, theme: 'dark' },
] as const

for (const view of VIEWS) {
  test(`shows the devices flow in ${view.theme} at ${view.name} width`, async ({ page, request, browser }) => {
    mkdirSync('e2e-screenshots', { recursive: true })
    const shoot = (state: string, target: Page) =>
      target.screenshot({ path: `e2e-screenshots/devices-${state}-${view.theme}-${view.name}.png`, fullPage: true, animations: 'disabled' })
    await page.setViewportSize({ width: view.width, height: view.height })
    await page.goto(`/devices?theme=${view.theme}`)
    await expect(page.getByRole('button', { name: 'Add this device' })).toBeVisible()
    await shoot('not-added', page)

    await page.getByRole('textbox', { name: 'Name for this device' }).fill('Work laptop')
    await page.getByRole('button', { name: 'Add this device' }).click()
    await expect(page.getByText(/Waiting for you to confirm this device/)).toBeVisible()
    await shoot('waiting', page)

    await confirmInTerminal(request, (await page.getByLabel('Pairing code of this device').textContent()) ?? '')
    await page.reload()
    await expect(page.getByText(/This device is added\./)).toBeVisible()

    const phoneContext = await browser.newContext({ baseURL: new URL(page.url()).origin })
    try {
      await askToBeAdded(await phoneContext.newPage(), 'Phone')
      await page.reload()
      await expect(page.getByRole('button', { name: 'The codes match: add it' })).toBeVisible()
      await shoot('confirm', page)
    } finally {
      await phoneContext.close()
    }
  })
}
