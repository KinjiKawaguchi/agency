/** A stand-in for the browser's `Notification`, which jsdom does not have: it records what was shown. */
export interface ShownNotification {
  title: string
  options?: NotificationOptions | undefined
  onclick: (() => void) | null
  closed: boolean
  close: () => void
}

export interface FakeNotifications {
  shown: ShownNotification[]
  setPermission: (permission: NotificationPermission) => void
  /** What the next permission prompt answers. */
  answer: (permission: NotificationPermission) => void
  asked: () => number
  restore: () => void
}

export function installFakeNotification(initial: NotificationPermission = 'default'): FakeNotifications {
  const shown: ShownNotification[] = []
  let permission = initial
  let nextAnswer: NotificationPermission = 'granted'
  let prompts = 0
  const original = (globalThis as { Notification?: unknown }).Notification
  class Fake implements ShownNotification {
    onclick: (() => void) | null = null
    closed = false
    constructor(
      readonly title: string,
      readonly options?: NotificationOptions,
    ) {
      shown.push(this)
    }
    close(): void {
      this.closed = true
    }
    static get permission(): NotificationPermission {
      return permission
    }
    static requestPermission(): Promise<NotificationPermission> {
      prompts += 1
      permission = nextAnswer
      return Promise.resolve(permission)
    }
  }
  Object.defineProperty(globalThis, 'Notification', { value: Fake, configurable: true, writable: true })
  return {
    shown,
    setPermission: (next) => void (permission = next),
    answer: (next) => void (nextAnswer = next),
    asked: () => prompts,
    restore: () => void Object.defineProperty(globalThis, 'Notification', { value: original, configurable: true, writable: true }),
  }
}

/** Makes the page look hidden or in front to the code that asks, until it is called again. */
export function setPageVisibility(state: 'hidden' | 'visible'): void {
  Object.defineProperty(document, 'visibilityState', { value: state, configurable: true })
}
