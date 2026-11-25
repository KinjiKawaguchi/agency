import { useQueryClient } from '@tanstack/react-query'
import { useCallback } from 'react'
import { useApi } from '../api/context'
import type { DecisionAnswer, DecisionResult } from '../api/clientApi'
import { devicesQuery, meQuery } from '../api/queries'
import { buildEnvelope, newEnvelopeId, signEnvelope } from './envelope'
import { deviceIdIn } from './enrollment'
import { useSettledDeviceKey } from './useDeviceKey'

/** How long a list of devices is trusted before a decision asks the node again. */
const DEVICES_FRESH_MS = 5_000

/**
 * Sends a decision. Signed by this device's key when the team's org has added it, which is how the node
 * wants decisions. A device that has no key or is not added yet falls back to the route where the home
 * node signs, so the owner can still decide before the first device is added.
 */
export function useDecide(): (answer: DecisionAnswer) => Promise<DecisionResult> {
  const api = useApi()
  const queryClient = useQueryClient()
  const settledKey = useSettledDeviceKey()
  return useCallback(
    async (answer) => {
      const held = await settledKey()
      if (held.status === 'ready') {
        const devices = await queryClient.fetchQuery({ ...devicesQuery(api), staleTime: DEVICES_FRESH_MS })
        const deviceId = deviceIdIn(devices, held.key.publicKey, answer.org)
        if (deviceId !== undefined) {
          const me = await queryClient.fetchQuery({ ...meQuery(api), staleTime: DEVICES_FRESH_MS })
          const envelope = buildEnvelope({ answer, account: me.account.id, id: newEnvelopeId(), at: new Date() })
          return api.decideSigned(answer.org, answer.team, {
            envelope: await signEnvelope(held.key, deviceId, envelope),
            title: answer.title,
            ...(answer.choiceLabel === undefined ? {} : { choiceLabel: answer.choiceLabel }),
          })
        }
      }
      return api.decide(answer)
    },
    [api, queryClient, settledKey],
  )
}
