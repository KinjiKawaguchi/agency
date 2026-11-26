import { useI18n } from '../i18n'
import { useOsNotices } from './osChoice'

/** The switch for notifications of the operating system, on the screen that holds this browser's own settings. */
export function OsNoticesCard() {
  const { t } = useI18n()
  const { support, isOn, turnOn, turnOff } = useOsNotices()
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
    </section>
  )
}
