import type { DeviceList, DeviceRequest } from '../api/clientApi'

export type Enrollment = { status: 'enrolled'; deviceId: string } | { status: 'pending'; request: DeviceRequest } | { status: 'none' }

/** Whether this device's key is among the account's devices, or waits to be added, or has not asked. */
export function enrollmentOf(publicKey: string, list: DeviceList): Enrollment {
  for (const org of list.orgs) {
    const device = org.devices.find((one) => one.publicKey === publicKey)
    if (device !== undefined) return { status: 'enrolled', deviceId: device.id }
  }
  const request = list.requests.find((one) => one.publicKey === publicKey)
  return request === undefined ? { status: 'none' } : { status: 'pending', request }
}

/** The device id this key has in `org`, when it was added there: only then does the org accept its signatures. */
export const deviceIdIn = (list: DeviceList, publicKey: string, org: string): string | undefined =>
  list.orgs.find((one) => one.org === org)?.devices.find((one) => one.publicKey === publicKey)?.id

/** The orgs that do not yet hold this key: the new device is added to each of them. */
export const orgsLacking = (list: DeviceList, publicKey: string): string[] =>
  list.orgs.filter((org) => !org.devices.some((one) => one.publicKey === publicKey)).map((org) => org.org)

/** Requests to add some other device, which this one can confirm. */
export const requestsFromOthers = (list: DeviceList, ownPublicKey: string): DeviceRequest[] => list.requests.filter((one) => one.publicKey !== ownPublicKey)
