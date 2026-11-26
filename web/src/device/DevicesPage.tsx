import { InstallCard } from '../pwa/InstallCard'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useApi } from '../api/context'
import type { DeviceList, DeviceRequest } from '../api/clientApi'
import { DEVICES_KEY, DEVICES_LIVE_MS, devicesQuery, meQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { useToast } from '../shared/Toasts'
import { addChangeFor, signChange } from './changes'
import { ApprovalSteps } from './ApprovalSteps'
import { deviceKeyCapabilities, type DeviceKey } from './deviceKey'
import { deviceIdIn, enrollmentOf, orgsLacking, requestsFromOthers } from './enrollment'
import { KeyKeepingNotes } from './KeyKeepingNotes'
import { useKeyKeeping, type KeyKeeping } from './keyKeeping'
import { useStanding } from '../guest/useStanding'
import { HomeNodesCard } from '../nodes/HomeNodesCard'
import { OsNoticesCard } from '../notifications/OsNoticesCard'
import { isHeldFor } from '../rootchange/held'
import { answerOf } from '../rootchange/provisional'
import { useDeviceKey } from './useDeviceKey'
import './device.css'


const messageOf = (failure: unknown): string => (failure instanceof Error ? failure.message : String(failure))

function AskToBeAdded({ deviceKey, onAsk }: { deviceKey: DeviceKey; onAsk: () => void }) {
  const { t } = useI18n()
  const api = useApi()
  const queryClient = useQueryClient()
  const [label, setLabel] = useState(t.devices.defaultLabel)
  const ask = useMutation({
    mutationFn: () => api.requestDevice({ label: label.trim(), publicKey: deviceKey.publicKey }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: DEVICES_KEY }),
  })
  return (
    <>
      <p className="device-lead">{t.devices.notAdded}</p>
      <label className="device-field">
        <span>{t.devices.labelField}</span>
        <input type="text" value={label} maxLength={80} onChange={(event) => setLabel(event.target.value)} />
      </label>
      <button className="btn primary" type="button" disabled={ask.isPending || label.trim() === ''} onClick={() => {
        // The click is what asks the browser to keep the key: some browsers show a prompt for it.
        onAsk()
        ask.mutate()
      }}>
        {ask.isPending ? t.devices.asking : t.devices.ask}
      </button>
      {ask.error === null ? null : <p className="device-invalid" role="alert">{t.devices.askFailed(messageOf(ask.error))}</p>}
    </>
  )
}

function PairingCode({ code, label, large = false }: { code: string; label: string; large?: boolean }) {
  return <p className={`pairing-code${large ? ' large' : ''}`} aria-label={label}>{code}</p>
}

function ThisDevice({ deviceKey, list }: { deviceKey: DeviceKey; list: DeviceList }) {
  const { t } = useI18n()
  const enrollment = enrollmentOf(deviceKey.publicKey, list)
  const kept: KeyKeeping = useKeyKeeping({ isEnrolled: enrollment.status === 'enrolled' })
  return (
    <section className="device-card">
      <h2 className="sub-head">{t.devices.thisDevice}</h2>
      <KeyKeepingNotes keeping={kept.keeping} isLost={kept.isLost} />
      <PairingCode code={deviceKey.pairingCode} label={t.devices.codeLabel} large={enrollment.status === 'pending'} />
      {enrollment.status === 'none' ? <AskToBeAdded deviceKey={deviceKey} onAsk={kept.ask} /> : null}
      {enrollment.status === 'pending' ? (
        <>
          <p className="device-lead" role="status">{t.devices.waitingLead}</p>
          <ApprovalSteps list={list} publicKey={deviceKey.publicKey} />
        </>
      ) : null}
      {enrollment.status === 'enrolled' ? (
        <p className="device-lead" role="status">
          {t.devices.added} {t.devices.addedAs(enrollment.deviceId)}
        </p>
      ) : null}
    </section>
  )
}

function RequestCard({ request, deviceKey, list }: { request: DeviceRequest; deviceKey: DeviceKey; list: DeviceList }) {
  const { t, fmt } = useI18n()
  const api = useApi()
  const queryClient = useQueryClient()
  const toast = useToast()
  const me = useQuery(meQuery(api))
  const pendingWords = `${t.devices.pendingRoot[0]}agency root-sign${t.devices.pendingRoot[1]}`
  const [heldHere, setHeld] = useState<readonly string[]>([]) /* stand-in-ok(device): what this click was told by the node, kept in this card; the node's own answer is read from `Me` below */
  // What this click was told, or else what the node says it holds: so that a reload does not offer the same approval again.
  const held = heldHere.length > 0 ? heldHere : me.data !== undefined && isHeldFor(me.data, request) ? [pendingWords] : []
  const confirm = useMutation({
    mutationFn: async () => {
      if (me.data === undefined) throw new Error(t.devices.accountUnknown)
      // Each org holds its own list of devices, so the new device is added to every org that lacks it,
      // and only an org that holds this device's key accepts its signature.
      const orgs = orgsLacking(list, request.publicKey).flatMap((org) => {
        const signerId = deviceIdIn(list, deviceKey.publicKey, org)
        return signerId === undefined ? [] : [{ org, signerId }]
      })
      if (orgs.length === 0) throw new Error(t.devices.nowhereToAdd)
      const waiting: string[] = []
      for (const { org, signerId } of orgs) {
        const result = await api.submitDeviceChange(await signChange(deviceKey, signerId, addChangeFor(request, org, me.data.account.id, new Date())))
        const answer = answerOf(result)
        if (answer.status === 'done') continue
        waiting.push(answer.status === 'pending' ? pendingWords : answer.message)
        // Kept as it comes, so that a later org's failure does not take away what the root's node already holds.
        setHeld([...waiting])
      }
      return waiting
    },
    // A change the root key's node holds for its owner is neither added nor refused: it is said as it is, not as a success.
    onSuccess: (waiting) => {
      if (waiting.length === 0) toast(t.devices.confirmed(request.label))
    },
    onError: (failure) => toast(t.devices.confirmFailed(messageOf(failure)), 'error'),
    onSettled: () => queryClient.invalidateQueries({ queryKey: DEVICES_KEY }),
  })
  return (
    <li className="device-request">
      <p className="device-request-head">
        <b>{request.label}</b> <span className="muted">{t.devices.requestedAt(fmt.relative(request.requestedAt, t.time.justNow))}</span>
      </p>
      <PairingCode code={request.pairingCode} label={t.devices.codeOf(request.label)} />
      <p className="device-compare-note">{t.devices.compare}</p>
      {held.length === 0 ? null : (
        <div className="device-lead" role="status">
          {[...new Set(held)].map((message) => (
            <p key={message}>{message}</p>
          ))}
        </div>
      )}
      <button className="btn primary" type="button" disabled={confirm.isPending || me.data === undefined || held.length > 0} onClick={() => confirm.mutate()}>
        {confirm.isPending ? t.devices.confirming : t.devices.confirm}
      </button>
    </li>
  )
}

function Requests({ deviceKey, list }: { deviceKey: DeviceKey; list: DeviceList }) {
  const { t } = useI18n()
  const requests = requestsFromOthers(list, deviceKey.publicKey)
  return (
    <section className="device-card">
      <h2 className="sub-head">{t.devices.requests}</h2>
      {requests.length === 0 ? (
        <p className="muted">{t.devices.noRequests}</p>
      ) : (
        <ul className="device-requests">
          {requests.map((request) => (
            <RequestCard key={request.publicKey} request={request} deviceKey={deviceKey} list={list} />
          ))}
        </ul>
      )}
    </section>
  )
}

function YourDevices({ deviceKey, list }: { deviceKey: DeviceKey; list: DeviceList }) {
  const { t, fmt } = useI18n()
  const seen = new Map(list.orgs.flatMap((org) => org.devices).map((device) => [device.id, device]))
  return (
    <section className="device-card">
      <h2 className="sub-head">{t.devices.yourDevices}</h2>
      <ul className="device-list">
        {[...seen.values()].map((device) => (
          <li key={device.id}>
            <b>{device.label}</b>
            {device.publicKey === deviceKey.publicKey ? <span className="device-here">{t.devices.thisOne}</span> : null}
            <span className="muted">{t.devices.addedOn(fmt.dateTime(device.addedAt))}</span>
          </li>
        ))}
      </ul>
    </section>
  )
}

function WithDeviceKey({ deviceKey }: { deviceKey: DeviceKey }) {
  const { t } = useI18n()
  const api = useApi()
  // While this screen is open the list is read every few seconds (not in a hidden tab), so a device that asks to be added, or that was approved, shows up by itself.
  const devices = useQuery(devicesQuery(api, DEVICES_LIVE_MS))
  if (devices.data === undefined) {
    return devices.error === null ? <p className="muted">{t.devices.loadingList}</p> : <p className="empty" role="alert">{t.devices.listFailed(messageOf(devices.error))}</p>
  }
  const enrollment = enrollmentOf(deviceKey.publicKey, devices.data)
  return (
    <>
      <ThisDevice deviceKey={deviceKey} list={devices.data} />
      {enrollment.status === 'enrolled' ? (
        <>
          <YourDevices deviceKey={deviceKey} list={devices.data} />
          <Requests deviceKey={deviceKey} list={devices.data} />
        </>
      ) : null}
    </>
  )
}

function Unsupported() {
  const { t } = useI18n()
  const found = deviceKeyCapabilities()
  const word = (has: boolean): string => (has ? t.devices.yes : t.devices.no)
  return (
    <>
      <p className="empty" role="alert">{t.devices.unsupported}</p>
      <p className="muted">{t.devices.capabilities(word(found.secureContext), word(found.webCrypto), word(found.indexedDb))}</p>
    </>
  )
}

/** This device's key and pairing code, asking to be added, and confirming other devices that ask. */
export function DevicesPage() {
  const { t } = useI18n()
  const key = useDeviceKey()
  const standing = useStanding()
  return (
    <>
      <h1 className="page-title">{t.devices.title}</h1>
      <p className="muted">{t.devices.intro}</p>
      {key.status === 'loading' ? <p className="muted">{t.devices.loading}</p> : null}
      {key.status === 'unsupported' ? <Unsupported /> : null}
      {key.status === 'failed' ? <p className="empty" role="alert">{t.devices.failed(key.reason)}</p> : null}
      {key.status === 'ready' ? <WithDeviceKey deviceKey={key.key} /> : null}
      {standing.status === 'guest' ? null : <HomeNodesCard />}
      <OsNoticesCard />
      <InstallCard />
    </>
  )
}
