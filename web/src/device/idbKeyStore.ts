import type { KeyStore } from './deviceKey'

const STORE = 'keys'
const KEY = 'device'

function request<T>(operation: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    operation.onsuccess = () => resolve(operation.result)
    operation.onerror = () => reject(operation.error ?? new Error('IndexedDB failed.'))
  })
}

function open(name: string): Promise<IDBDatabase> {
  const opening = indexedDB.open(name, 1)
  opening.onupgradeneeded = () => opening.result.createObjectStore(STORE)
  return request(opening)
}

/** Keeps the key pair in this origin's IndexedDB, which stores a non-extractable key without exposing its bytes. */
export function idbKeyStore(name = 'agency-device'): KeyStore {
  return {
    load: async () => {
      const database = await open(name)
      try {
        const stored: unknown = await request(database.transaction(STORE).objectStore(STORE).get(KEY))
        return isKeyPair(stored) ? stored : undefined
      } finally {
        database.close()
      }
    },
    save: async (pair) => {
      const database = await open(name)
      try {
        await request(database.transaction(STORE, 'readwrite').objectStore(STORE).put(pair, KEY))
      } finally {
        database.close()
      }
    },
  }
}

const isKeyPair = (value: unknown): value is CryptoKeyPair =>
  typeof value === 'object' && value !== null && 'privateKey' in value && 'publicKey' in value
