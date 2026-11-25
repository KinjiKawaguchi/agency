import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, type RouterHistory } from '@tanstack/react-router'
import { useState } from 'react'
import { KeyStoreProvider } from '../device/context'
import type { KeyStore } from '../device/deviceKey'
import { ApiProvider } from '../api/context'
import { createApiClient, type ApiClient } from '../api/client'
import { I18nProvider } from '../i18n'
import type { Locale } from '../i18n/locale'
import { SignInExpiredProvider } from '../shared/SignInExpired'
import { ToastProvider } from '../shared/Toasts'
import { createAppRouter } from './router'

interface AppProps {
  client?: ApiClient
  history?: RouterHistory
  locale?: Locale
  /** Where this device's key is kept; the browser's IndexedDB when absent. */
  keyStore?: KeyStore
}

export function App({ client, history, locale, keyStore }: AppProps) {
  const [api] = useState(() => client ?? createApiClient())
  const [queryClient] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: true } } }))
  const [router] = useState(() => createAppRouter(history))
  return (
    <I18nProvider {...(locale === undefined ? {} : { initial: locale })}>
      <QueryClientProvider client={queryClient}>
        <ApiProvider client={api}>
          <SignInExpiredProvider>
            <KeyStoreProvider {...(keyStore === undefined ? {} : { store: keyStore })}>
              <ToastProvider>
                <RouterProvider router={router} />
              </ToastProvider>
            </KeyStoreProvider>
          </SignInExpiredProvider>
        </ApiProvider>
      </QueryClientProvider>
    </I18nProvider>
  )
}
