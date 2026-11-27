import type { Envelope, Signed } from '../api/clientApi'
import type { OutboxEntry } from './entry'
import { loadOutbox, OUTBOX_STORAGE_KEY, saveOutbox } from './storage'

const envelope = { body: { id: 'e-1' }, signer: 'alice', signature: 'sig' } as unknown as Signed<Envelope>
const entry = (overrides: Partial<OutboxEntry> = {}): OutboxEntry => ({
  id: 'e-1', org: 'acme', team: 'app', itemKey: 'acme:app:r-1', itemTitle: 'Ship it?', text: 'Go.', draft: { text: 'Go.', choice: 'a' }, envelope, createdAt: '2026-11-27T01:00:00Z', attempts: 0, phase: 'sending', ...overrides,
})
const memory = () => {
  const held = new Map<string, string>()
  return { held, getItem: (key: string) => held.get(key) ?? null, setItem: (key: string, value: string) => void held.set(key, value), removeItem: (key: string) => void held.delete(key) }
}

describe('the decisions kept in the browser', () => {
  it('are read back as they were kept, in their order', () => {
    const storage = memory()
    saveOutbox([entry({ id: 'e-1' }), entry({ id: 'e-2', envelope: { ...envelope, body: { id: 'e-2' } } as unknown as Signed<Envelope>, phase: 'failed', reason: 'refused' })], storage)
    expect(loadOutbox(storage).map((one) => [one.id, one.phase])).toEqual([['e-1', 'sending'], ['e-2', 'failed']])
  })

  it('leave nothing behind when there are none', () => {
    const storage = memory()
    saveOutbox([entry()], storage)
    saveOutbox([], storage)
    expect(storage.held.has(OUTBOX_STORAGE_KEY)).toBe(false)
  })

  it('leave out an entry that is not one, and keep the others, never repairing it', () => {
    const storage = memory()
    const wrongId = { ...entry({ id: 'e-3' }) }
    storage.held.set(OUTBOX_STORAGE_KEY, JSON.stringify([entry(), { id: 'x' }, 7, null, { ...entry({ id: 'e-4' }), phase: 'lost' }, { ...entry(), envelope: undefined }, wrongId, { ...entry(), draft: { text: 1, choice: null } }]))
    expect(loadOutbox(storage).map((one) => one.id)).toEqual(['e-1'])
  })

  it('are none when the storage is empty, says something that is not a list, or cannot be read', () => {
    const storage = memory()
    expect(loadOutbox(storage)).toEqual([])
    storage.held.set(OUTBOX_STORAGE_KEY, '{"a":1}')
    expect(loadOutbox(storage)).toEqual([])
    storage.held.set(OUTBOX_STORAGE_KEY, 'not json')
    expect(loadOutbox(storage)).toEqual([])
    expect(loadOutbox({ getItem: () => { throw new Error('denied') } })).toEqual([])
    expect(loadOutbox(undefined)).toBeDefined()
  })

  it('are kept in memory only, without an error, when the storage refuses', () => {
    expect(() => saveOutbox([entry()], { setItem: () => { throw new Error('full') }, removeItem: () => undefined })).not.toThrow()
  })
})
