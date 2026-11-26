import { Link, useRouterState } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { DEVICES_LIVE_MS, devicesPoll, devicesQuery, POLL_MS } from '../api/queries'
import { useI18n } from '../i18n'
import { enrollmentOf } from './enrollment'
import { useDeviceKey } from './useDeviceKey'
import './device.css'

/**
 * On any screen but the devices one: tells a person whose device is not added yet to add it, in one
 * sentence that says why, or that it waits for a confirmation. Silent once the device is added, when
 * the browser cannot make keys, or while the node has not answered.
 */
export function AddDeviceNotice() {
  const { t } = useI18n()
  const api = useApi()
  const key = useDeviceKey()
  // While this device is not added, the list is read often, so the notice changes when it asks and goes when it is approved.
  const devices = useQuery({ ...devicesQuery(api), refetchInterval: (query) => devicesPoll(query, query.state.data !== undefined && key.status === 'ready' && enrollmentOf(key.key.publicKey, query.state.data).status !== 'enrolled' ? DEVICES_LIVE_MS : POLL_MS) })
  const path = useRouterState({ select: (state) => state.location.pathname })
  if (key.status !== 'ready' || devices.data === undefined || path === '/devices') return null
  const { status } = enrollmentOf(key.key.publicKey, devices.data)
  if (status === 'enrolled') return null
  return (
    <div className="device-notice" role="status">
      <p>{status === 'pending' ? t.devices.promptPending : t.devices.promptNone}</p>
      <Link className="btn primary" to="/devices">
        {status === 'pending' ? t.devices.promptSee : t.devices.promptAdd}
      </Link>
    </div>
  )
}
