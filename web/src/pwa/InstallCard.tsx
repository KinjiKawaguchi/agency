import { useI18n } from '../i18n'
import { isSafariOnMac, useInstall } from './useInstall'
import './install.css'

/** Offers to install Agency as an app on this device, where the browser can: on Android, Windows, Linux, and in Chromium on a Mac. */
export function InstallCard() {
  const { t } = useI18n()
  const { canInstall, isInstalled, install } = useInstall()
  if (isInstalled) return null
  if (!canInstall && !isSafariOnMac()) return null
  return (
    <section className="install-card" aria-labelledby="install-title">
      <h2 id="install-title" className="sub-head">{t.install.title}</h2>
      <p className="muted">{t.install.lead}</p>
      {canInstall ? (
        <button className="btn primary" type="button" onClick={() => void install()}>
          {t.install.button}
        </button>
      ) : (
        <p>{t.install.safari}</p>
      )}
    </section>
  )
}
