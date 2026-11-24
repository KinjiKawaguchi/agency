export const LOCALES = ['en', 'ja'] as const
export type Locale = (typeof LOCALES)[number]

export const DEFAULT_LOCALE: Locale = 'en'

/** Endonyms, so a person can find their language whatever the current one is. */
export const LOCALE_NAMES: Record<Locale, string> = { en: 'English', ja: '日本語' }

const STORAGE_KEY = 'agency.locale'

export const isLocale = (value: unknown): value is Locale => LOCALES.some((locale) => locale === value)

function readStored(): string | null {
  try {
    return window.localStorage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}

export function storeLocale(locale: Locale): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, locale)
  } catch {
    // Storage can be blocked; the choice then lasts for this page only.
  }
}

/** `?lang=` wins, then the stored choice, then the browser language, then English. */
export function detectLocale(): Locale {
  const fromQuery = new URLSearchParams(window.location.search).get('lang')
  if (isLocale(fromQuery)) return fromQuery
  const stored = readStored()
  if (isLocale(stored)) return stored
  const fromBrowser = window.navigator.languages.map((tag) => tag.split('-')[0]).find(isLocale)
  return fromBrowser ?? DEFAULT_LOCALE
}
