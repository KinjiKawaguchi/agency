import type { Me } from '../api/clientApi'
import { deviceIdOf } from '../device/fingerprint'
import { rootChangesOf } from './provisional'

/**
 * Whether the root's node holds the approval of this device's request for its owner to sign: a held `addDevice` of this
 * device's id, or, when the node names no device, one with its label for this account. Read from `Me.rootChanges`, which the
 * node gives its owner, so it is known where this node is the root's node, and after a reload as before it.
 */
export function isHeldFor(me: Me, request: { label: string; pairingCode: string }): boolean {
  return rootChangesOf(me).some((change) => {
    if (change.kind !== 'addDevice') return false
    if (change.device !== undefined) return change.device === deviceIdOf(request.pairingCode)
    return change.label === request.label && change.member === me.account.id
  })
}
