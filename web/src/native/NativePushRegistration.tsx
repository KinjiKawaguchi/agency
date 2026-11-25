import { useEffect, useRef } from 'react'
import { useApi } from '../api/context'
import { NATIVE_PUSH_EVENT, pushOfPage, readPush, type NativePush } from './push'

/**
 * Inside the iPhone app: hands the phone's push token to the node, once the owner has allowed
 * notifications and Apple has issued one, and again whenever it changes. It reads the token from the
 * app's bridge, sends it, and keeps nothing: not in storage, not in the log. A call that fails is tried
 * again the next time the page loads or the app announces the token. Outside the app it does nothing.
 */
export function NativePushRegistration() {
  const api = useApi()
  // The body last sent in this page, only so one token is not sent twice here. It is not persisted.
  const lastSent = useRef<string | null>(null)

  useEffect(() => {
    const register = (push: NativePush | undefined): void => {
      if (push === undefined) return
      const body = JSON.stringify(push)
      if (lastSent.current === body) return
      lastSent.current = body
      api.registerPushToken(push).catch(() => {
        lastSent.current = null
      })
    }
    const onAnnounced = (event: Event): void => register(readPush((event as CustomEvent<unknown>).detail) ?? pushOfPage(window))
    register(pushOfPage(window))
    window.addEventListener(NATIVE_PUSH_EVENT, onAnnounced)
    return () => window.removeEventListener(NATIVE_PUSH_EVENT, onAnnounced)
  }, [api])

  return null
}
