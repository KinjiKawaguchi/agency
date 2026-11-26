import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { useApi } from '../api/context'
import { useI18n } from '../i18n'
import { forgetKeptFiles } from '../pwa/register'

const ExpiredContext = createContext(false)

/** Whether the node's calls are failing because the sign-in expired. Clears when a call works again. */
export const useSignInExpired = (): boolean => useContext(ExpiredContext)

export function SignInExpiredProvider({ children }: { children: ReactNode }) {
  const api = useApi()
  const [isExpired, setIsExpired] = useState(false)
  useEffect(
    () =>
      api.onSession((state) => {
        setIsExpired(state === 'expired')
        // The sign-in ran out: the installed app's worker keeps nothing from before it.
        if (state === 'expired') forgetKeptFiles()
      }),
    [api],
  )
  return <ExpiredContext.Provider value={isExpired}>{children}</ExpiredContext.Provider>
}

/**
 * Says the sign-in ran out and offers to sign in again. It never reloads by itself: what the viewer has
 * written stays on the page until they choose to.
 */
export function SignInExpiredBanner() {
  const { t } = useI18n()
  if (!useSignInExpired()) return null
  return (
    <div className="signin-notice" role="alert">
      <p>{t.app.signInExpired.message}</p>
      <button className="btn" type="button" onClick={() => window.location.reload()}>
        {t.app.signInExpired.reload}
      </button>
    </div>
  )
}
