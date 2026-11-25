import { useEffect, useState } from 'react'
import { canMakeDeviceKeys, loadOrCreateDeviceKey, type DeviceKey } from './deviceKey'
import { useKeyStore } from './context'

export type DeviceKeyState = { status: 'loading' } | { status: 'unsupported' } | { status: 'failed'; reason: string } | { status: 'ready'; key: DeviceKey }

/** This device's key: made on first use and kept in the browser. */
export function useDeviceKey(): DeviceKeyState {
  const store = useKeyStore()
  const [state, setState] = useState<DeviceKeyState>({ status: 'loading' })
  useEffect(() => {
    let isCurrent = true
    const keep = (next: DeviceKeyState): void => {
      if (isCurrent) setState(next)
    }
    void (async () => {
      if (!(await canMakeDeviceKeys())) return keep({ status: 'unsupported' })
      try {
        keep({ status: 'ready', key: await loadOrCreateDeviceKey(store) })
      } catch (failure) {
        keep({ status: 'failed', reason: failure instanceof Error ? failure.message : String(failure) })
      }
    })()
    return () => {
      isCurrent = false
    }
  }, [store])
  return state
}
