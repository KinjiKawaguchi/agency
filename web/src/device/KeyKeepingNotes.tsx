import { useI18n } from '../i18n'
import type { Keeping } from './keyKeeping'

/** What the person should know about this browser's key: that an earlier one is gone, and whether the browser will keep this one. */
export function KeyKeepingNotes({ keeping, isLost }: { keeping: Keeping; isLost: boolean }) {
  const { t } = useI18n()
  const text = t.devices.keyKeeping
  return (
    <>
      {isLost ? <p className="device-invalid" role="alert">{text.lost}</p> : null}
      <p className="muted">{keeping === 'kept' ? text.kept : text.atRisk}</p>
    </>
  )
}
