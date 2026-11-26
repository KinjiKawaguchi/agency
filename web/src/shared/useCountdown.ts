import { useEffect, useState } from 'react'

/** The whole seconds left until `at` (a time in ms), counted down once a second; 0 once it has passed. */
export function useSecondsUntil(at: number, now: () => number = Date.now): number {
  const [current, setCurrent] = useState(() => now())
  useEffect(() => {
    const timer = window.setInterval(() => setCurrent(now()), 1000)
    return () => window.clearInterval(timer)
  }, [now])
  return Math.max(0, Math.ceil((at - current) / 1000))
}
