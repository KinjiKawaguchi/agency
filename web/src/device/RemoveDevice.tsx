import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useApi } from '../api/context'
import type { Device, DeviceList } from '../api/clientApi'
import { DEVICES_KEY, meQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { answerOf } from '../rootchange/changes'
import { useToast } from '../shared/Toasts'
import { signChange } from './changes'
import type { DeviceKey } from './deviceKey'
import { pairingCodeOf } from './fingerprint'
import { isRemovable, removalPlanOf, removeChangeFor, type RemovalPlan } from './removal'

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
  if (plan.lastIn.length > 0) return <p className="muted device-remove-note">{words.last(device.label, plan.lastIn.join(', '))}</p>
  if (!isRemovable(plan)) return null
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

function Confirmation({ device, plan, ownKey, onDone, onCancel }: Pick<Props, 'device' | 'ownKey'> & { plan: RemovalPlan; onDone: (messages: readonly string[]) => void; onCancel: () => void }) {
  const { t } = useI18n()
  const words = t.devices.remove
  const api = useApi()
  const queryClient = useQueryClient()
  const toast = useToast()
  const me = useQuery(meQuery(api))
  const code = useQuery({ queryKey: ['pairing-code', device.publicKey], queryFn: () => pairingCodeOf(device.publicKey), staleTime: Infinity })
  const pendingWords = `${words.pendingRoot[0]}agency root-sign${words.pendingRoot[1]}`
  const remove = useMutation({
    mutationFn: async () => {
      if (me.data === undefined) throw new Error(t.devices.accountUnknown)
      const waiting: string[] = []
      for (const { org, deviceId, signerId } of plan.signedIn) {
        if (signerId === undefined) continue
        const result = await api.submitDeviceChange(await signChange(ownKey, signerId, removeChangeFor(org, me.data.account.id, deviceId, new Date())))
        const answer = answerOf(result)
        if (answer.status !== 'done') waiting.push(answer.status === 'pending' ? pendingWords : answer.message)
      }
      return waiting
    },
    onSuccess: (waiting) => {
      const left = plan.unsignedIn.length > 0 ? [words.stillIn(plan.unsignedIn.join(', '))] : []
      if (waiting.length === 0) toast(words.removed(device.label))
      onDone([...new Set([...waiting, ...left])])
    },
    onError: (failure) => toast(words.failed(messageOf(failure)), 'error'),
    onSettled: () => queryClient.invalidateQueries({ queryKey: DEVICES_KEY }),
  })
  return (
    <div className="device-remove-panel" role="group" aria-label={words.title(device.label)}>
      <p><b>{words.title(device.label)}</b></p>
      {code.data === undefined ? null : <p>{words.body(device.label, code.data)}</p>}
      {plan.orgs.length > 1 ? <p>{words.fromOrgs(plan.orgs.map((one) => one.org).join(', '))}</p> : null}
      {plan.isThisDevice ? <p className="device-invalid">{words.thisDevice}</p> : null}
      {plan.unsignedIn.length > 0 ? <p className="muted">{words.stillIn(plan.unsignedIn.join(', '))}</p> : null}
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
