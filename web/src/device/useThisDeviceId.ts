import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { devicesQuery } from '../api/queries'
import { enrollmentOf } from './enrollment'
import { useDeviceKey } from './useDeviceKey'

/**
 * The id this browser's device has in the account's orgs (`Device.id`), once the node lists the key; nothing before that, and nothing
 * for a browser that has no key or is not added. A registration that names its device (a push subscription) takes this.
 */
export function useThisDeviceId(): string | undefined {
  const key = useDeviceKey()
  const devices = useQuery({ ...devicesQuery(useApi()), enabled: key.status === 'ready' })
  if (key.status !== 'ready' || devices.data === undefined) return undefined
  const enrollment = enrollmentOf(key.key.publicKey, devices.data)
  return enrollment.status === 'enrolled' ? enrollment.deviceId : undefined
}
