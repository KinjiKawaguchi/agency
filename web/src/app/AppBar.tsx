import { Link, useRouterState } from '@tanstack/react-router'
import { useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { devicesQuery, useTeams } from '../api/queries'
import { enrollmentOf } from '../device/enrollment'
import { useDeviceKey } from '../device/useDeviceKey'
import { countWaitingOnOwner } from '../desk/items'
import { useI18n } from '../i18n'
import { LOCALES, LOCALE_NAMES, isLocale } from '../i18n/locale'
import { isNativeApp } from '../nodes/homeNodes'
import { useHomeNodes } from '../nodes/useHomeNodes'
import { Face } from '../people/Face'
import type { Messages } from '../i18n/en'
import { THEMES, useTheme, type Theme } from '../theme/useTheme'
import './shell.css'

const THEME_LABELS = (t: ReturnType<typeof useI18n>['t']): Record<Theme, string> => ({
  auto: t.settings.themeAuto,
  light: t.settings.themeLight,
  dark: t.settings.themeDark,
})

type PagePath = '/desk' | '/overview' | '/org' | '/people' | '/costs' | '/evaluation' | '/live'

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
      { to: '/live', label: (t) => t.nav.live },
    ],
  },
]

const isOn = (path: string, to: PagePath): boolean => path === to || path.startsWith(`${to}/`)

export function AppBar() {
  const { t, locale, setLocale } = useI18n()
  const { theme, setTheme } = useTheme()
  const { me, teams } = useTeams()
  const { nodes } = useHomeNodes()
  const api = useApi()
  const key = useDeviceKey()
  const devices = useQuery(devicesQuery(api))
  // The devices this one can approve: other devices' requests, and only once this device is added in some org.
  const approvals = devices.data === undefined || key.status !== 'ready' || enrollmentOf(key.key.publicKey, devices.data).status !== 'enrolled' ? 0 : devices.data.requests.filter((request) => request.publicKey !== key.key.publicKey).length
  // Once the owner lists another home node, every tab says which one it is, so the work node cannot be taken for the personal one.
  const nodeName = nodes.others.length === 0 || isNativeApp() ? undefined : (nodes.self ?? window.location.host)
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
        {nodeName === undefined ? null : <Link className="node-chip" to="/devices" title={t.devices.homeNodes.title}>{nodeName}</Link>}
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
          <Link className="me-link" to="/devices" aria-label={approvals > 0 ? `${t.nav.devices}, ${t.notices.devicesMark(approvals)}` : t.nav.devices} title={t.nav.devices}>
            {me === undefined ? t.nav.devices : <Face label={me.account.label} kind="human" size="sm" />}
            {approvals > 0 ? <span className="me-count" aria-hidden="true">{approvals}</span> : null}
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
