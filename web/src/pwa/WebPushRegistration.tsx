import { useEffect } from 'react'
import { useApi } from '../api/context'
import { registerHeldSubscription } from './useWebPush'
import { useWebPushBrowser } from './WebPushBrowserContext'

/**
 * Keeps the node told of this browser's push subscription: when any page of the app opens, and when the worker says
 * the browser renewed it. It draws nothing, asks for no permission, and does nothing in a browser that holds no subscription.
 */
export function WebPushRegistration() {
  const api = useApi()
  const browser = useWebPushBrowser()
  useEffect(() => {
    const register = (): void => void registerHeldSubscription(api, browser).catch(() => undefined)
    register()
    return browser.support().hasServiceWorker ? browser.onSubscriptionChanged(register) : undefined
  }, [api, browser])
  return null
}
