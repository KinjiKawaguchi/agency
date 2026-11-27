import type { DeviceChange, DeviceList } from '../api/clientApi'
import { deviceIdIn } from './enrollment'
import { timestampOf } from './time'

/** One org that lists the device being removed. */
export interface OrgRemoval {
  org: string
  /** The id the device has in this org. */
  deviceId: string
  /** The id this browser's own device has in this org, which signs the removal; none when this browser is not added there. */
  signerId: string | undefined
  /** The account has no other device in this org, so nothing there could sign after this one is gone. */
  isLast: boolean
}

/** What removing one device means, org by org (docs/glossary.md, "Remove a device"). */
export interface RemovalPlan {
  orgs: readonly OrgRemoval[]
  /** The orgs in which it is the only device: the removal is refused as a whole while there is one. */
  lastIn: readonly string[]
  /** The orgs this browser cannot sign in, which keep listing the device. */
  unsignedIn: readonly string[]
  /** The orgs the removal is sent to, in the order the node lists them. */
  signedIn: readonly OrgRemoval[]
  /** Whether the device being removed is the one signing. */
  isThisDevice: boolean
}

export function removalPlanOf(list: DeviceList, devicePublicKey: string, ownPublicKey: string): RemovalPlan {
  const orgs = list.orgs.flatMap((one): OrgRemoval[] => {
    const device = one.devices.find((candidate) => candidate.publicKey === devicePublicKey)
    if (device === undefined) return []
    return [{ org: one.org, deviceId: device.id, signerId: deviceIdIn(list, ownPublicKey, one.org), isLast: one.devices.length === 1 }]
  })
  return {
    orgs,
    lastIn: orgs.filter((one) => one.isLast).map((one) => one.org),
    unsignedIn: orgs.filter((one) => one.signerId === undefined).map((one) => one.org),
    signedIn: orgs.filter((one) => one.signerId !== undefined),
    isThisDevice: devicePublicKey === ownPublicKey,
  }
}

/** Whether the device may be removed from here: it is listed somewhere, and is the last one nowhere. */
export const isRemovable = (plan: RemovalPlan): boolean => plan.orgs.length > 0 && plan.lastIn.length === 0 && plan.signedIn.length > 0

/** The change that removes a device (by its id in that org) from the account's devices in one org. */
export const removeChangeFor = (org: string, account: string, deviceId: string, at: Date): DeviceChange => ({ org, account, op: 'remove', device: deviceId, at: timestampOf(at) })
