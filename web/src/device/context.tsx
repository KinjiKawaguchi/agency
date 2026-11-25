import { createContext, useContext, type ReactNode } from 'react'
import type { KeyStore } from './deviceKey'
import { idbKeyStore } from './idbKeyStore'

const KeyStoreContext = createContext<KeyStore | null>(null)

/** Where this device's key is kept. Tests give it a store in memory, as jsdom has no IndexedDB. */
export function KeyStoreProvider({ store, children }: { store?: KeyStore; children: ReactNode }) {
  return <KeyStoreContext.Provider value={store ?? null}>{children}</KeyStoreContext.Provider>
}

/** The store provided, or the browser's IndexedDB when none was. */
export const useKeyStore = (): KeyStore => useContext(KeyStoreContext) ?? browserStore

const browserStore = idbKeyStore()
