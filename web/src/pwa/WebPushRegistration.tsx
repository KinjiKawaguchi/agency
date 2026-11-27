import { useEffect } from 'react'
import { useApi } from '../api/context'
import { useThisDeviceId } from '../device/useThisDeviceId'
import { registerHeldSubscription } from './useWebPush'
import { useWebPushBrowser } from './WebPushBrowserContext'

/**
 * Keeps the node told of this browser's push subscription: when any page of the app opens, and when the worker says
 * the browser renewed it. It draws nothing, asks for no permission, and does nothing in a browser that holds no subscription.
 */
export function WebPushRegistration() {
  const api = useApi()
  const browser = useWebPushBrowser()
  // Once this browser is added, the node is told which device the subscription belongs to, so that removing the device stops its notices.
  const device = useThisDeviceId()
  useEffect(() => {
    const register = (): void => void registerHeldSubscription(api, browser, device).catch(() => undefined)
    register()
    return browser.support().hasServiceWorker ? browser.onSubscriptionChanged(register) : undefined
  }, [api, browser, device])
  return null
}
