import { useEffect } from 'react'
import { useWaitingCount } from '../desk/useWaitingCount'
import { useSignInExpired } from '../shared/SignInExpired'
import { setNativeBadge } from './badge'

/**
 * Inside the iPhone app: keeps the icon's badge at the number of requests waiting for the owner, the
 * number the desk's tab shows. It says nothing while that number is not known for certain (still
 * loading, a team that could not be read, the sign-in expired): a wrong 0 would hide waiting requests.
 */
export function NativeBadge() {
  const waiting = useWaitingCount()
  const isExpired = useSignInExpired()
  useEffect(() => {
    if (waiting !== undefined && !isExpired) setNativeBadge(window, waiting)
  }, [waiting, isExpired])
  return null
}
