import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { useApi } from '../api/context'
import type { DecisionAnswer, DecisionResult } from '../api/clientApi'
import { devicesQuery, meQuery } from '../api/queries'
import { rememberSent } from '../notifications/sent'
import { buildEnvelope, newEnvelopeId, signEnvelope } from './envelope'
import { deviceIdIn } from './enrollment'
import { useDeviceKey, useSettledDeviceKey } from './useDeviceKey'

/** How long a list of devices is trusted before a decision asks the node again. */
const DEVICES_FRESH_MS = 5_000

/** This device cannot sign a decision: its browser cannot make keys, or it is not added to the team's org. */
export class CannotSignError extends Error {
  readonly reason: 'unsupported' | 'not-added'

  constructor(reason: 'unsupported' | 'not-added') {
    super(reason === 'unsupported' ? 'This browser cannot make the key that signs decisions.' : 'This device is not added to this organization.')
    this.name = 'CannotSignError'
    this.reason = reason
  }
}

/**
 * Sends a decision, signed by this device's key: the only way the node takes one. A device that has no
 * key, or is not added to the team's org, cannot decide; it says why, and the caller keeps the draft.
 */
export function useDecide(): (answer: DecisionAnswer) => Promise<DecisionResult> {
  const api = useApi()
  const queryClient = useQueryClient()
  const settledKey = useSettledDeviceKey()
  return useCallback(
    async (answer) => {
      const held = await settledKey()
      if (held.status !== 'ready') throw new CannotSignError('unsupported')
      const devices = await queryClient.fetchQuery({ ...devicesQuery(api), staleTime: DEVICES_FRESH_MS })
      const deviceId = deviceIdIn(devices, held.key.publicKey, answer.org)
      if (deviceId === undefined) throw new CannotSignError('not-added')
      const me = await queryClient.fetchQuery({ ...meQuery(api), staleTime: DEVICES_FRESH_MS })
      const envelope = buildEnvelope({ answer, account: me.account.id, id: newEnvelopeId(), at: new Date() })
      rememberSent(envelope.id)
      return api.decideSigned(answer.org, answer.team, {
        envelope: await signEnvelope(held.key, deviceId, envelope),
        title: answer.title,
        ...(answer.choiceLabel === undefined ? {} : { choiceLabel: answer.choiceLabel }),
      })
    },
    [api, queryClient, settledKey],
  )
}

/** Whether this device can sign for `org`, as far as is known: `checking` while the key or the node's list is still coming. */
export type SigningState = 'checking' | 'ready' | 'not-added' | 'unsupported'

export function useSigningState(org: string): SigningState {
  const api = useApi()
  const key = useDeviceKey()
  const devices = useQuery(devicesQuery(api))
  if (key.status === 'unsupported' || key.status === 'failed') return 'unsupported'
  if (key.status !== 'ready' || devices.data === undefined) return 'checking'
  return deviceIdIn(devices.data, key.key.publicKey, org) === undefined ? 'not-added' : 'ready'
}

