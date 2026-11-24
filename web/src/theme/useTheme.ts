import { useCallback, useEffect, useState } from 'react'

export const THEMES = ['auto', 'light', 'dark'] as const
export type Theme = (typeof THEMES)[number]

const STORAGE_KEY = 'agency.theme'

const isTheme = (value: unknown): value is Theme => THEMES.some((theme) => theme === value)

function readTheme(): Theme {
  const fromQuery = new URLSearchParams(window.location.search).get('theme')
  if (isTheme(fromQuery)) return fromQuery
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY)
    return isTheme(stored) ? stored : 'auto'
  } catch {
    return 'auto'
  }
}

function applyTheme(theme: Theme): void {
  if (theme === 'auto') document.documentElement.removeAttribute('data-theme')
  else document.documentElement.setAttribute('data-theme', theme)
}

export function useTheme(): { theme: Theme; setTheme: (theme: Theme) => void } {
  const [theme, setThemeState] = useState<Theme>(readTheme)
  useEffect(() => applyTheme(theme), [theme])
  const setTheme = useCallback((next: Theme) => {
    try {
      window.localStorage.setItem(STORAGE_KEY, next)
    } catch {
      // Storage can be blocked; the choice then lasts for this page only.
    }
    setThemeState(next)
  }, [])
  return { theme, setTheme }
}
