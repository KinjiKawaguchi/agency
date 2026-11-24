import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import { en, type Messages } from './en'
import { createFormatters, type Formatters } from './format'
import { ja } from './ja'
import { detectLocale, storeLocale, type Locale } from './locale'

const CATALOGS: Record<Locale, Messages> = { en, ja }

interface I18n {
  locale: Locale
  setLocale: (locale: Locale) => void
  t: Messages
  fmt: Formatters
}

const I18nContext = createContext<I18n | null>(null)

export function I18nProvider({ initial, children }: { initial?: Locale; children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => initial ?? detectLocale())
  const value = useMemo<I18n>(
    () => ({
      locale,
      setLocale: (next) => {
        storeLocale(next)
        setLocaleState(next)
      },
      t: CATALOGS[locale],
      fmt: createFormatters(locale),
    }),
    [locale],
  )
  useEffect(() => {
    document.documentElement.lang = locale
  }, [locale])
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}

export function useI18n(): I18n {
  const value = useContext(I18nContext)
  if (value === null) throw new Error('useI18n must be used inside I18nProvider')
  return value
}
