import { useEffect, useState } from 'react'

/**
 * The current time, kept moving while `isActive`: every animation frame when `mode` is 'frame', otherwise
 * every `intervalMs`. It stops when nothing needs it, so a screen at rest costs nothing.
 */
export function useTicker(isActive: boolean, mode: 'frame' | number): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!isActive) return undefined
    setNow(Date.now())
    if (mode === 'frame') {
      let id = 0
      const frame = (): void => {
        setNow(Date.now())
        id = window.requestAnimationFrame(frame)
      }
      id = window.requestAnimationFrame(frame)
      return () => window.cancelAnimationFrame(id)
    }
    const timer = window.setInterval(() => setNow(Date.now()), mode)
    return () => window.clearInterval(timer)
  }, [isActive, mode])
  return now
}
