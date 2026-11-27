import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useApi } from '../api/context'
import type { Device, DeviceList } from '../api/clientApi'
import { DEVICES_KEY, decisionsQuery, devicesQuery, meQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { useOutbox } from '../outbox/OutboxProvider'
import { answerOf } from '../rootchange/changes'
import { useToast } from '../shared/Toasts'
import { signChange } from './changes'
import type { DeviceKey } from './deviceKey'
import { pairingCodeOf } from './fingerprint'
import { isRemovable, removalPlanOf, removeChangeFor, type RemovalPlan } from './removal'
import { useReplaceDeviceKey } from './useDeviceKey'

const messageOf = (failure: unknown): string => (failure instanceof Error ? failure.message : String(failure))

interface Props {
  device: Device
  list: DeviceList
  ownKey: DeviceKey
}

/**
 * The way to remove one of the account's devices: a button, then a confirmation that names the device by its label and pairing
 * code and says it can never sign again. This browser signs the removal for every org that lists the device (one change each,
 * docs/glossary.md "Remove a device"); the device leaves the list when the node stops listing it, never before.
 */
export function RemoveDevice({ device, list, ownKey }: Props) {
  const { t } = useI18n()
  const words = t.devices.remove
  const plan = removalPlanOf(list, device.publicKey, ownKey.publicKey)
  const [isAsking, setAsking] = useState(false)
  const [told, setTold] = useState<readonly string[]>([]) /* stand-in-ok(device): what this click was told by the node, kept in this row */
  const kept = keptBy(plan, useOutbox().entries)
  if (kept > 0) return <p className="muted device-remove-note">{words.keptFirst(kept)}</p>
  if (plan.lastIn.length > 0) return <p className="muted device-remove-note">{words.last(device.label, plan.lastIn.join(', '))}</p>
  if (!isRemovable(plan)) {
    // Listed only where this browser cannot sign: it cannot be removed from here, and the row says where it stays listed (also right after a removal that left it there).
    const lines = [...new Set([...told, ...(plan.unsignedIn.length > 0 ? [words.stillIn(plan.unsignedIn.join(', '))] : [])])]
    return lines.length === 0 ? null : <div className="device-remove">{lines.map((message) => <p key={message} className="muted device-remove-note">{message}</p>)}</div>
  }
  return (
    <div className="device-remove">
      {isAsking ? (
        <Confirmation device={device} plan={plan} ownKey={ownKey} onDone={(messages) => { setAsking(false); setTold(messages) }} onCancel={() => setAsking(false)} />
      ) : (
        <button className="btn small" type="button" aria-label={words.buttonFor(device.label)} onClick={() => { setTold([]); setAsking(true) }}>
          {words.button}
        </button>
      )}
      {told.map((message) => (
        <p key={message} className="device-lead" role="status">{message}</p>
      ))}
    </div>
  )
}

/**
 * How many decisions kept in this browser for sending were signed by the device being removed (only this browser's own device ever signed
 * one here): its key is revoked by the removal, so they could never be taken (docs/protocol.md, "Devices": a removal takes effect for envelopes still waiting too).
 */
export const keptBy = (plan: RemovalPlan, entries: ReadonlyArray<{ envelope: { signer: string } }>): number =>
  entries.filter((entry) => plan.orgs.some((one) => one.deviceId === entry.envelope.signer)).length

function Confirmation({ device, plan, ownKey, onDone, onCancel }: Pick<Props, 'device' | 'ownKey'> & { plan: RemovalPlan; onDone: (messages: readonly string[]) => void; onCancel: () => void }) {
  const { t } = useI18n()
  const words = t.devices.remove
  const api = useApi()
  const queryClient = useQueryClient()
  const toast = useToast()
  const me = useQuery(meQuery(api))
  const replaceKey = useReplaceDeviceKey()
  const isRemoved = useRef(false)
  // What the node holds for a team that has not taken it yet ("Saved, not delivered"): a decision signed by the device being removed is
  // refused when the node tries again. The log does not say which device signed which, so the confirmation says how many there are.
  const log = useQuery(decisionsQuery(api))
  const queued = log.data === undefined ? undefined : log.data.entries.filter((entry) => entry.state.kind === 'queued').length
  const code = useQuery({ queryKey: ['pairing-code', device.publicKey], queryFn: () => pairingCodeOf(device.publicKey), staleTime: Infinity })
  const pendingWords = `${words.pendingRoot[0]}agency root-sign${words.pendingRoot[1]}`
  const remove = useMutation({
    mutationFn: async () => {
      if (me.data === undefined) throw new Error(t.devices.accountUnknown)
      const fresh = removalPlanOf(await queryClient.fetchQuery({ ...devicesQuery(api), staleTime: 0 }), device.publicKey, ownKey.publicKey)
      // Already gone from every org that listed it: nothing is left to sign.
      if (fresh.orgs.length === 0) {
        isRemoved.current = true
        if (fresh.isThisDevice) await replaceKey()
        return { waiting: [], removedFrom: [], stillIn: [] }
      }
      if (!isRemovable(fresh)) throw new Error(fresh.lastIn.length > 0 ? words.last(device.label, fresh.lastIn.join(', ')) : words.stillIn(fresh.unsignedIn.join(', ')))
      const waiting: string[] = []
      for (const { org, deviceId, signerId } of fresh.signedIn) {
        if (signerId === undefined) continue
        const result = await api.submitDeviceChange(await signChange(ownKey, signerId, removeChangeFor(org, me.data.account.id, deviceId, new Date())))
        const answer = answerOf(result)
        if (answer.status !== 'done') waiting.push(answer.status === 'pending' ? pendingWords : answer.message)
      }
      isRemoved.current = waiting.length === 0
      // The org has revoked this browser's key once the removal is done (not while it is only held): it makes a new one, and the
      // screen offers to add that, never the revoked one.
      if (fresh.isThisDevice && waiting.length === 0) await replaceKey()
      return { waiting, removedFrom: fresh.signedIn.map((one) => one.org), stillIn: [...fresh.unsignedIn] }
    },
    onSuccess: ({ waiting, removedFrom, stillIn }) => {
      const left = stillIn.length > 0 ? [words.stillIn(stillIn.join(', '))] : []
      // Where the device is removed and where it is still listed (this browser cannot sign there) are said apart: "can never sign again" is said only of the orgs it left.
      if (waiting.length === 0) toast(stillIn.length > 0 ? words.removedFrom(device.label, removedFrom.join(', ')) : plan.isThisDevice ? words.removedThis : words.removed(device.label))
      onDone([...new Set([...waiting, ...left])])
    },
    onError: (failure) => toast(isRemoved.current ? words.newKeyFailed(messageOf(failure)) : words.failed(messageOf(failure)), 'error'),
    onSettled: () => queryClient.invalidateQueries({ queryKey: DEVICES_KEY }),
  })
  return (
    <div className="device-remove-panel" role="group" aria-label={words.title(device.label)}>
      <p><b>{words.title(device.label)}</b></p>
      {code.data === undefined ? null : <p>{words.body(device.label, code.data)}</p>}
      {plan.orgs.length > 1 ? <p>{words.fromOrgs(plan.orgs.map((one) => one.org).join(', '))}</p> : null}
      {plan.isThisDevice ? <p className="device-invalid">{words.thisDevice}</p> : null}
      {plan.unsignedIn.length > 0 ? <p className="muted">{words.stillIn(plan.unsignedIn.join(', '))}</p> : null}
      {queued === undefined || queued === 0 ? null : <p>{plan.isThisDevice ? words.queuedHere(queued) : words.queuedOther(queued, device.label)}</p>}
      <div className="device-remove-actions">
        <button className="btn primary" type="button" disabled={remove.isPending || me.data === undefined || code.data === undefined} onClick={() => remove.mutate()}>
          {remove.isPending ? words.removing : words.confirm}
        </button>
        <button className="btn" type="button" disabled={remove.isPending} onClick={onCancel}>
          {words.cancel}
        </button>
      </div>
    </div>
  )
}
