import type { Notice, NoticeTarget } from './present'
import { isOsNoticeOn } from './osChoice'

const ICON = '/icons/icon-192.png'

/** The page a target opens, as the router is told it: one request is opened on the desk by its key. */
export const navigationOf = (target: NoticeTarget): { to: '/desk'; search: { item: string } } | { to: '/desk' | '/people' | '/devices' } =>
  target.to === '/desk' && target.item !== undefined ? { to: '/desk', search: { item: target.item } } : { to: target.to }

/** Whether a notice should reach the operating system now: the viewer asked for it, and the page is not in front of them. */
export const shouldTellOs = (): boolean => isOsNoticeOn() && document.visibilityState === 'hidden'

/**
 * Shows a notice as a notification of the operating system, in the words the page's own notice has. A click brings
 * the tab to the front and opens what the notice leads to. Nothing about it leaves this browser.
 */
export function showOsNotice(notice: Notice, title: string, open: (target: NoticeTarget) => void): void {
  try {
    const shown = new Notification(title, { body: notice.message, icon: ICON, ...(notice.tag === undefined ? {} : { tag: notice.tag }) })
    shown.onclick = () => {
      window.focus()
      shown.close()
      open(notice.target)
    }
  } catch {
    // A browser that cannot make one from a page (an Android browser asks for a worker) throws; the page's own notice was still shown.
  }
}
