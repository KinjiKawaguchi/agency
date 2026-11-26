import { useCallback, useEffect, useState } from 'react'

/** What Chromium-based browsers hand over when the app can be installed. */
interface InstallPromptEvent extends Event {
  prompt: () => Promise<void>
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>
}

const isStandalone = (): boolean =>
  (typeof window.matchMedia === 'function' && window.matchMedia('(display-mode: standalone)').matches) || (navigator as { standalone?: boolean }).standalone === true

/** Safari on a Mac installs through its File menu, and has no prompt to offer. */
export const isSafariOnMac = (): boolean => /^Apple/.test(navigator.vendor) && /Macintosh/.test(navigator.userAgent) && !('ontouchend' in document)

export interface Install {
  /** The browser offers to install the app now. */
  canInstall: boolean
  /** The page runs as the installed app. */
  isInstalled: boolean
  install: () => Promise<void>
}

/** Whether this page can be installed as an app, and the way to do it: the browser's own prompt, shown when the viewer asks. */
export function useInstall(): Install {
  const [offer, setOffer] = useState<InstallPromptEvent | undefined>(undefined)
  const [isInstalled, setIsInstalled] = useState(isStandalone)

  useEffect(() => {
    const onOffer = (event: Event): void => {
      // The browser's own mini-bar would pop up by itself: the viewer asks from the Devices screen instead.
      event.preventDefault()
      setOffer(event as InstallPromptEvent)
    }
    const onInstalled = (): void => {
      setOffer(undefined)
      setIsInstalled(true)
    }
    window.addEventListener('beforeinstallprompt', onOffer)
    window.addEventListener('appinstalled', onInstalled)
    return () => {
      window.removeEventListener('beforeinstallprompt', onOffer)
      window.removeEventListener('appinstalled', onInstalled)
    }
  }, [])

  const install = useCallback(async (): Promise<void> => {
    if (offer === undefined) return
    await offer.prompt()
    const { outcome } = await offer.userChoice
    // An offer can be used once, accepted or not.
    setOffer(undefined)
    if (outcome === 'accepted') setIsInstalled(true)
  }, [offer])

  return { canInstall: offer !== undefined && !isInstalled, isInstalled, install }
}
