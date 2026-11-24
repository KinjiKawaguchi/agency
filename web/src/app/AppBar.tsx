import { Link } from '@tanstack/react-router'
import { useEffect } from 'react'
import { useTeamViews } from '../api/queries'
import { openItems } from '../desk/items'
import { useI18n } from '../i18n'
import { LOCALES, LOCALE_NAMES, isLocale } from '../i18n/locale'
import { Face } from '../people/Face'
import { THEMES, useTheme, type Theme } from '../theme/useTheme'
import './shell.css'

const THEME_LABELS = (t: ReturnType<typeof useI18n>['t']): Record<Theme, string> => ({
  auto: t.settings.themeAuto,
  light: t.settings.themeLight,
  dark: t.settings.themeDark,
})

export function AppBar() {
  const { t, locale, setLocale } = useI18n()
  const { theme, setTheme } = useTheme()
  const { me, views } = useTeamViews()
  const waiting = me === undefined ? 0 : openItems(views).filter((item) => item.isMine).length
  const themeLabels = THEME_LABELS(t)

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
          <Link to="/desk">
            {t.nav.desk}
            {waiting > 0 ? <span className="nav-count">{waiting}</span> : null}
          </Link>
          <Link to="/people">{t.nav.people}</Link>
          <Link to="/org">{t.nav.org}</Link>
          <Link to="/costs">{t.nav.costs}</Link>
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
          {me === undefined ? null : <Face label={me.account.label} kind="human" size="sm" title={me.account.label} />}
        </div>
      </div>
    </header>
  )
}
