import { useCallback, useEffect, useState } from 'react'

const MADE = 'agency.deviceKeyMade'
const LOST = 'agency.deviceKeyLost'

/** Whether the browser promises to keep this origin's data (the device key among it): it does, may not, or cannot say. */
export type Keeping = 'kept' | 'at-risk' | 'unknown'

interface Persistence {
  persisted?: () => Promise<boolean>
  persist?: () => Promise<boolean>
}

const persistence = (): Persistence | undefined => (typeof navigator === 'undefined' ? undefined : (navigator.storage as Persistence | undefined))

async function answerOf(ask: (() => Promise<boolean>) | undefined): Promise<Keeping> {
  if (ask === undefined) return 'unknown'
  try {
    return (await ask()) ? 'kept' : 'at-risk'
  } catch {
    return 'unknown'
  }
}

/** What the browser has promised so far; it asks for nothing, so it can be read as the page opens. */
export const keepingNow = (): Promise<Keeping> => answerOf(persistence()?.persisted?.bind(persistence()))

/**
 * Asks the browser to keep this origin's data. A person's click should be what asks: some browsers show a prompt,
 * and some decide without one (an installed app, a bookmarked or often used site is kept, others may not be).
 */
export const askToKeep = (): Promise<Keeping> => answerOf(persistence()?.persist?.bind(persistence()))

function read(key: string): string | null {
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

function write(key: string, value: string | undefined): void {
  try {
    if (value === undefined) window.localStorage.removeItem(key)
    else window.localStorage.setItem(key, value)
  } catch {
    // Storage can be blocked; nothing is then remembered between visits.
  }
}

/**
 * Notes that this browser has a device key now. A key that had to be made while an earlier one was noted is a
 * lost one: its site data was cleared, or the browser removed it. Clearing every site's data clears this note
 * too, and that is told apart from a first visit by nothing; only the part of it that leaves this note (a browser
 * that removed the key's database but kept the origin's other data) is seen.
 */
export function recordKeyLoad(isCreated: boolean): void {
  if (isCreated && read(MADE) !== null) write(LOST, 'yes')
  write(MADE, 'yes')
}

export const wasKeyLost = (): boolean => read(LOST) !== null

export const forgetKeyLost = (): void => write(LOST, undefined)

export interface KeyKeeping {
  keeping: Keeping
  /** Asks the browser to keep the key; call it from a click. */
  ask: () => void
  isLost: boolean
}

/** What this page knows of whether the key will be kept, and whether an earlier one was lost until an added key is back. */
export function useKeyKeeping({ isEnrolled }: { isEnrolled: boolean }): KeyKeeping {
  const [keeping, setKeeping] = useState<Keeping>('unknown')
  const [isLost, setIsLost] = useState(wasKeyLost)
  useEffect(() => {
    let isCurrent = true
    void keepingNow().then((now) => isCurrent && setKeeping(now))
    return () => {
      isCurrent = false
    }
  }, [])
  useEffect(() => {
    if (!isEnrolled || !isLost) return
    forgetKeyLost()
    setIsLost(false)
  }, [isEnrolled, isLost])
  const ask = useCallback(() => void askToKeep().then(setKeeping), [])
  return { keeping, ask, isLost }
}
