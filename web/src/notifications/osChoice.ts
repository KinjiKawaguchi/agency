import { useCallback, useSyncExternalStore } from 'react'

const STORAGE_KEY = 'agency.osNotices'
const listeners = new Set<() => void>()

/** What this browser can do about notifications of the operating system: nothing, or the state of the permission for this page. */
export type OsSupport = 'unsupported' | NotificationPermission

export const osSupport = (): OsSupport => (typeof Notification === 'undefined' ? 'unsupported' : Notification.permission)

function readChoice(): boolean {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === 'on'
  } catch {
    return false
  }
}

function writeChoice(isOn: boolean): void {
  try {
    if (isOn) window.localStorage.setItem(STORAGE_KEY, 'on')
    else window.localStorage.removeItem(STORAGE_KEY)
  } catch {
    // Storage can be blocked; the choice then lasts for this page only, through the permission.
  }
  for (const listener of listeners) listener()
}

/** Whether the viewer asked for them in this browser and the browser still allows them. */
export const isOsNoticeOn = (): boolean => readChoice() && osSupport() === 'granted'

/** Calls `listener` when the permission changes in the browser's own settings, until the returned function is called. */
function watchPermission(listener: () => void): () => void {
  let status: PermissionStatus | undefined
  let isStopped = false
  void navigator.permissions
    ?.query({ name: 'notifications' })
    .then((found) => {
      if (isStopped) return
      status = found
      found.addEventListener('change', listener)
    })
    .catch(() => undefined)
  return () => {
    isStopped = true
    status?.removeEventListener('change', listener)
  }
}

const subscribe = (listener: () => void): (() => void) => {
  listeners.add(listener)
  const stopWatching = watchPermission(listener)
  return () => {
    listeners.delete(listener)
    stopWatching()
  }
}

export interface OsNoticeChoice {
  support: OsSupport
  isOn: boolean
  /** Asks the browser for the permission, which it only gives to a click, and turns them on when it is given. */
  turnOn: () => Promise<void>
  turnOff: () => void
}

/** The viewer's choice to be told, by the operating system, what is new while the page is hidden. It is per origin: each home node has its own. */
export function useOsNotices(): OsNoticeChoice {
  // What it reads is one string of both, so that a change of either (the permission, the choice) draws again.
  useSyncExternalStore(subscribe, () => `${osSupport()}/${readChoice()}`)
  const turnOn = useCallback(async (): Promise<void> => {
    if (osSupport() === 'unsupported') return
    const permission = await Notification.requestPermission()
    writeChoice(permission === 'granted')
  }, [])
  const turnOff = useCallback(() => writeChoice(false), [])
  return { support: osSupport(), isOn: isOsNoticeOn(), turnOn, turnOff }
}
