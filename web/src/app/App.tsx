import { QueryClientProvider, type QueryClient } from '@tanstack/react-query'
import { RouterProvider, type RouterHistory } from '@tanstack/react-router'
import { useState } from 'react'
import { KeyStoreProvider } from '../device/context'
import type { KeyStore } from '../device/deviceKey'
import { DeviceKeyProvider } from '../device/useDeviceKey'
import { ApiProvider } from '../api/context'
import { createApiClient, type ApiClient } from '../api/client'
import { I18nProvider } from '../i18n'
import type { Locale } from '../i18n/locale'
import { NativeBadge } from '../native/NativeBadge'
import { NativePushRegistration } from '../native/NativePushRegistration'
import { SignInExpiredProvider } from '../shared/SignInExpired'
import { WebPushBrowserProvider } from '../pwa/WebPushBrowserContext'
import { WebPushRegistration } from '../pwa/WebPushRegistration'
import type { WebPushBrowser } from '../pwa/useWebPush'
import { OutboxProvider } from '../outbox/OutboxProvider'
import { createTelemetry } from '../telemetry/telemetry'
import { TelemetryReporting } from '../telemetry/TelemetryReporting'
import { ToastProvider } from '../shared/Toasts'
import { createQueryClient } from './queryClient'
import { createAppRouter } from './router'

interface AppProps {
  client?: ApiClient
  history?: RouterHistory
  locale?: Locale
  /** Where this device's key is kept; the browser's IndexedDB when absent. */
  keyStore?: KeyStore
  /** The client every read goes through; one with the app's retry rule when absent. */
  queryClient?: QueryClient
  /** What Web Push uses of the browser; the real one when absent. */
  webPushBrowser?: WebPushBrowser
}

export function App({ client, history, locale, keyStore, queryClient: given, webPushBrowser }: AppProps) {
  const [telemetry] = useState(createTelemetry)
  const [api] = useState(() => client ?? createApiClient(undefined, undefined, { onSample: telemetry.record }))
  const [queryClient] = useState(() => given ?? createQueryClient())
  const [router] = useState(() => createAppRouter(history))
  return (
    <I18nProvider {...(locale === undefined ? {} : { initial: locale })}>
      <QueryClientProvider client={queryClient}>
        <ApiProvider client={api}>
          <SignInExpiredProvider>
            <KeyStoreProvider {...(keyStore === undefined ? {} : { store: keyStore })}>
              <DeviceKeyProvider>
                <ToastProvider>
                  <WebPushBrowserProvider browser={webPushBrowser}>
                    <OutboxProvider>
                      <NativePushRegistration />
                      <WebPushRegistration />
                      <NativeBadge />
                      <TelemetryReporting telemetry={telemetry} />
                      <RouterProvider router={router} />
                    </OutboxProvider>
                  </WebPushBrowserProvider>
                </ToastProvider>
              </DeviceKeyProvider>
            </KeyStoreProvider>
          </SignInExpiredProvider>
        </ApiProvider>
      </QueryClientProvider>
    </I18nProvider>
  )
}
