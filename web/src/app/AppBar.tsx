import { Link, useRouterState } from '@tanstack/react-router'
import { useEffect } from 'react'
import { useTeams } from '../api/queries'
import { countWaitingOnOwner } from '../desk/items'
import { useI18n } from '../i18n'
import { LOCALES, LOCALE_NAMES, isLocale } from '../i18n/locale'
import { Face } from '../people/Face'
import type { Messages } from '../i18n/en'
import { THEMES, useTheme, type Theme } from '../theme/useTheme'
import './shell.css'

const THEME_LABELS = (t: ReturnType<typeof useI18n>['t']): Record<Theme, string> => ({
  auto: t.settings.themeAuto,
  light: t.settings.themeLight,
  dark: t.settings.themeDark,
})

type PagePath = '/desk' | '/overview' | '/org' | '/people' | '/costs' | '/evaluation'

interface Group {
  key: 'desk' | 'overview' | 'organization' | 'results'
  /** The pages of the group, the one a click on the group opens first. */
  pages: ReadonlyArray<{ to: PagePath; label: (t: Messages) => string }>
  label: (t: Messages) => string
}

/** The main menu has one entry per group, so a phone shows four; a group of several pages lists them in a menu of its own. */
const GROUPS: readonly Group[] = [
  { key: 'desk', label: (t) => t.nav.desk, pages: [{ to: '/desk', label: (t) => t.nav.desk }] },
  { key: 'overview', label: (t) => t.nav.overview, pages: [{ to: '/overview', label: (t) => t.nav.overview }] },
  {
    key: 'organization',
    label: (t) => t.nav.organization,
    pages: [
      { to: '/org', label: (t) => t.nav.org },
      { to: '/people', label: (t) => t.nav.people },
    ],
  },
  {
    key: 'results',
    label: (t) => t.nav.results,
    pages: [
      { to: '/costs', label: (t) => t.nav.costs },
      { to: '/evaluation', label: (t) => t.nav.evaluation },
    ],
  },
]

const isOn = (path: string, to: PagePath): boolean => path === to || path.startsWith(`${to}/`)

export function AppBar() {
  const { t, locale, setLocale } = useI18n()
  const { theme, setTheme } = useTheme()
  const { me, teams } = useTeams()
  const waiting = me === undefined ? 0 : countWaitingOnOwner(teams)
  const themeLabels = THEME_LABELS(t)
  const path = useRouterState({ select: (state) => state.location.pathname })
  const current = GROUPS.find((group) => group.pages.some((page) => isOn(path, page.to)))

  useEffect(() => {
    document.title = waiting > 0 ? `(${waiting}) ${t.app.name}` : t.app.name
  }, [waiting, t.app.name])

  return (
    <header className="appbar">
      <div className="appbar-in">
        <Link className="brand" to="/desk">
          <span className="brand-seal" aria-hidden="true" />
          {t.app.name}
        </Link>
        <nav className="mainnav" aria-label={t.nav.label}>
          {GROUPS.map((group) => (
            <Link key={group.key} to={group.pages[0]?.to ?? '/desk'} aria-current={group === current ? 'page' : undefined}>
              {group.label(t)}
              {group.key === 'desk' && waiting > 0 ? <span className="nav-count">{waiting}</span> : null}
            </Link>
          ))}
        </nav>
        <div className="appbar-right">
          <select aria-label={t.settings.theme} value={theme} onChange={(event) => setTheme(THEMES.find((one) => one === event.target.value) ?? 'auto')}>
            {THEMES.map((one) => (
              <option key={one} value={one}>{themeLabels[one]}</option>
            ))}
          </select>
          <select aria-label={t.settings.language} value={locale} onChange={(event) => isLocale(event.target.value) && setLocale(event.target.value)}>
            {LOCALES.map((one) => (
              <option key={one} value={one}>{LOCALE_NAMES[one]}</option>
            ))}
          </select>
          <Link className="me-link" to="/devices" aria-label={t.nav.devices} title={t.nav.devices}>
            {me === undefined ? t.nav.devices : <Face label={me.account.label} kind="human" size="sm" />}
          </Link>
        </div>
        {current === undefined || current.pages.length < 2 ? null : (
          <nav className="subnav" aria-label={t.nav.section(current.label(t))}>
            {current.pages.map((page) => (
              <Link key={page.to} to={page.to}>
                {page.label(t)}
              </Link>
            ))}
          </nav>
        )}
      </div>
    </header>
  )
}
