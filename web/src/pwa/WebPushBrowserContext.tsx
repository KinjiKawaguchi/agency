import { createContext, useContext, type ReactNode } from 'react'
import { realBrowser, type WebPushBrowser } from './useWebPush'

const WebPushBrowserContext = createContext<WebPushBrowser>(realBrowser)

/** What Web Push uses of the browser. Tests give a stand-in, since jsdom has no push service. */
export function WebPushBrowserProvider({ browser, children }: { browser?: WebPushBrowser | undefined; children: ReactNode }) {
  return <WebPushBrowserContext.Provider value={browser ?? realBrowser}>{children}</WebPushBrowserContext.Provider>
}

export const useWebPushBrowser = (): WebPushBrowser => useContext(WebPushBrowserContext)
