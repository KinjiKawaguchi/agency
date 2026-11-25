import type { DeviceChange, DeviceRequest, Signed } from '../api/clientApi'
import type { DeviceKey } from './deviceKey'
import { signDocument } from './signing'
import { timestampOf } from './time'

/** The change that adds the requesting device to one org. */
export const addChangeFor = (request: DeviceRequest, org: string, account: string, at: Date): DeviceChange => ({
  org,
  account,
  op: 'add',
  device: { id: request.deviceId, label: request.label, publicKey: request.publicKey, addedAt: timestampOf(at) },
  at: timestampOf(at),
})

/** The add change signed by this device, named by `signerDeviceId`. */
export const signChange = (key: DeviceKey, signerDeviceId: string, change: DeviceChange): Promise<Signed<DeviceChange>> => signDocument(key, 'device', change, signerDeviceId)
