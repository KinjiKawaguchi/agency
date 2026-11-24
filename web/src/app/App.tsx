import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, type RouterHistory } from '@tanstack/react-router'
import { useState } from 'react'
import { ApiProvider } from '../api/context'
import { createApiClient, type ApiClient } from '../api/client'
import { I18nProvider } from '../i18n'
import type { Locale } from '../i18n/locale'
import { ToastProvider } from '../shared/Toasts'
import { createAppRouter } from './router'

interface AppProps {
  client?: ApiClient
  history?: RouterHistory
  locale?: Locale
}

export function App({ client, history, locale }: AppProps) {
  const [api] = useState(() => client ?? createApiClient())
  const [queryClient] = useState(() => new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: true } } }))
  const [router] = useState(() => createAppRouter(history))
  return (
    <I18nProvider {...(locale === undefined ? {} : { initial: locale })}>
      <QueryClientProvider client={queryClient}>
        <ApiProvider client={api}>
          <ToastProvider>
            <RouterProvider router={router} />
          </ToastProvider>
        </ApiProvider>
      </QueryClientProvider>
    </I18nProvider>
  )
}
