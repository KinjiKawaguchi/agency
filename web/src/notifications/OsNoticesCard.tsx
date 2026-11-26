import { useApi } from '../api/context'
import { useI18n } from '../i18n'
import { useWebPushBrowser } from '../pwa/WebPushBrowserContext'
import { useWebPush, type WebPushState } from '../pwa/useWebPush'
import type { PushDelivery } from '../pwa/webPush'
import { Skeleton } from '../shared/Skeleton'
import '../pwa/push.css'
import { useOsNotices } from './osChoice'

/**
 * The second switch: the node tells this browser through its push service, with no page open. Nothing is drawn where
 * the browser has no Web Push (or it is an iPhone, which has the app) or the node has none; where the browser refuses
 * notifications, the first switch says so for both.
 */
function Failing({ delivery }: { delivery: PushDelivery }) {
  const { t, fmt } = useI18n()
  const text = t.notices.os.push
  if (delivery.kind !== 'failing') return null
  return (
    <p role="status" className="push-failing">
      {text.isFailing(fmt.dateTime(delivery.failure.at), delivery.failure.reason)} {delivery.lastTakenAt === undefined ? text.neverTaken : text.lastTaken(fmt.dateTime(delivery.lastTakenAt))}
    </p>
  )
}

function WebPushSwitch({ state, turnOn, turnOff }: { state: WebPushState; turnOn: () => void; turnOff: () => void }) {
  const { t } = useI18n()
  const text = t.notices.os.push
  if (state.kind === 'unsupported' || state.kind === 'unavailable' || state.kind === 'denied') return null
  return (
    <div className="push-switch" aria-labelledby="web-push">
      <h3 className="sub-head" id="web-push">{text.title}</h3>
      <p className="muted">{text.lead}</p>
      {state.kind === 'loading' ? <Skeleton kind="short" /> : null}
      {state.kind === 'on' ? <p>{text.isOn}</p> : null}
      {state.kind === 'on' ? <Failing delivery={state.delivery} /> : null}
      {state.kind === 'stale' ? <p>{text.isStale}</p> : null}
      {state.kind === 'failed' ? <p role="alert">{text.failed(state.message)}</p> : null}
      {state.kind === 'loading' ? null : (
        <button className="btn" type="button" onClick={state.kind === 'on' ? turnOff : turnOn}>
          {state.kind === 'on' ? text.turnOff : state.kind === 'stale' ? text.subscribeAgain : text.turnOn}
        </button>
      )}
    </div>
  )
}

/** The switches for notifications of the operating system, on the screen that holds this browser's own settings. */
export function OsNoticesCard() {
  const { t } = useI18n()
  const { support, isOn, turnOn, turnOff } = useOsNotices()
  const webPush = useWebPush(useApi(), useWebPushBrowser())
  const text = t.notices.os
  const state = support === 'unsupported' ? text.isUnsupported : support === 'denied' ? text.isBlocked : isOn ? text.isOn : undefined
  return (
    <section className="device-card" aria-labelledby="os-notices">
      <h2 className="sub-head" id="os-notices">{text.title}</h2>
      <p className="muted">{text.lead}</p>
      {state === undefined ? null : <p>{state}</p>}
      {support === 'unsupported' || support === 'denied' ? null : (
        <button className="btn" type="button" onClick={isOn ? turnOff : () => void turnOn()}>
          {isOn ? text.turnOff : text.turnOn}
        </button>
      )}
      <WebPushSwitch state={webPush.state} turnOn={webPush.turnOn} turnOff={webPush.turnOff} />
    </section>
  )
}
