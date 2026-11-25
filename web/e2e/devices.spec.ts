import { expect, test } from '@playwright/test'
import { resetNode, trackErrors } from './helpers'

// The device key from the protocol's test vectors, and its pairing code (testdata/vectors.json).
const VECTOR_KEY = '/RckOFqgx1tk+3jNYC+h2ZH96/drE8WO1wLqyDXp9hg='
const VECTOR_CODE = 'dbc2-9825-1c51'
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

test('shows the code of another device’s key, the one the protocol vectors give', async ({ page }) => {
  await page.goto('/devices')
  await page.getByRole('textbox', { name: 'Public key of the other device' }).fill(VECTOR_KEY)
  await expect(page.getByLabel('Pairing code of the other device')).toHaveText(VECTOR_CODE)
})
