import { useI18n } from '../i18n'
import type { QueuedCount } from './queuedCount'

interface Props {
  count: QueuedCount
  /** Whether it is the browser's own device that goes. */
  isThisDevice: boolean
  label: string
}

/**
 * What the removal will cost among the decisions the node holds undelivered: the ones the device signed (they are refused once its key is
 * revoked) and the ones whose signer the node did not record. When it recorded no signer for any of them, only the total is known.
 */
export function QueuedDecisions({ count, isThisDevice, label }: Props) {
  const { t } = useI18n()
  const words = t.devices.remove
  const { signedByDevice, notRecorded, byOthers, isAtLeast } = count
  if (signedByDevice + notRecorded + byOthers === 0) return null
  if (signedByDevice + byOthers === 0) return <p>{isThisDevice ? words.queuedHere(notRecorded, isAtLeast) : words.queuedOther(notRecorded, isAtLeast, label)}</p>
  return (
    <>
      {signedByDevice === 0 ? null : <p>{isThisDevice ? words.signedHere(signedByDevice, isAtLeast) : words.signedBy(signedByDevice, isAtLeast, label)}</p>}
      {notRecorded === 0 ? null : <p>{isThisDevice ? words.notRecordedHere(notRecorded, isAtLeast) : words.notRecordedBy(notRecorded, isAtLeast, label)}</p>}
    </>
  )
}
