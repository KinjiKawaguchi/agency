import { createContext, createElement, useContext, useEffect, useState, type ReactNode } from 'react'
import { canMakeDeviceKeys, loadDeviceKey, type DeviceKey, type KeyStore } from './deviceKey'
import { recordKeyLoad } from './keyKeeping'
import { useKeyStore } from './context'

export type DeviceKeyState = { status: 'loading' } | { status: 'unsupported' } | { status: 'failed'; reason: string } | { status: 'ready'; key: DeviceKey }

const loadings = new WeakMap<KeyStore, Promise<DeviceKeyState>>()

/**
 * This device's key, made on first use and kept in the browser. A store is read once: two callers that
 * ask at the same time must get the same key, not make two and keep one.
 */
export function settledDeviceKey(store: KeyStore): Promise<DeviceKeyState> {
  const existing = loadings.get(store)
  if (existing !== undefined) return existing
  const loading = (async (): Promise<DeviceKeyState> => {
    if (!(await canMakeDeviceKeys())) return { status: 'unsupported' }
    try {
      const { key, isCreated } = await loadDeviceKey(store)
      recordKeyLoad(isCreated)
      return { status: 'ready', key }
    } catch (failure) {
      return { status: 'failed', reason: failure instanceof Error ? failure.message : String(failure) }
    }
  })()
  loadings.set(store, loading)
  return loading
}

interface Held {
  state: DeviceKeyState
  /** Resolves once the key is made or known to be impossible; for code that must not act before then. */
  settled: () => Promise<DeviceKeyState>
}

const DeviceKeyContext = createContext<Held | null>(null)

/** Loads this device's key once for the whole page. */
export function DeviceKeyProvider({ children }: { children: ReactNode }) {
  const store = useKeyStore()
  const [state, setState] = useState<DeviceKeyState>({ status: 'loading' })
  useEffect(() => {
    let isCurrent = true
    void settledDeviceKey(store).then((settled) => isCurrent && setState(settled))
    return () => {
      isCurrent = false
    }
  }, [store])
  return createElement(DeviceKeyContext.Provider, { value: { state, settled: () => settledDeviceKey(store) } }, children)
}

function useHeld(): Held {
  const held = useContext(DeviceKeyContext)
  if (held === null) throw new Error('useDeviceKey must be used inside DeviceKeyProvider')
  return held
}

export const useDeviceKey = (): DeviceKeyState => useHeld().state

export const useSettledDeviceKey = (): (() => Promise<DeviceKeyState>) => useHeld().settled
