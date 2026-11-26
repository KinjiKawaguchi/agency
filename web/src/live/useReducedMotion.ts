import { useEffect, useState } from 'react'

const QUERY = '(prefers-reduced-motion: reduce)'

/** Whether the viewer asked for less motion: then nothing on the screen moves by itself. Follows the setting while the page is open. */
export function useReducedMotion(): boolean {
  const [isReduced, setIsReduced] = useState(() => typeof window.matchMedia === 'function' && window.matchMedia(QUERY).matches)
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return undefined
    const query = window.matchMedia(QUERY)
    const listen = (): void => setIsReduced(query.matches)
    query.addEventListener('change', listen)
    return () => query.removeEventListener('change', listen)
  }, [])
  return isReduced
}
