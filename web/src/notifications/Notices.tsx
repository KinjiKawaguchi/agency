import { useQuery } from '@tanstack/react-query'
import { useRouterState } from '@tanstack/react-router'
import { useEffect, useRef } from 'react'
import { useApi } from '../api/context'
import { decisionsQuery, devicesQuery, useTeamStates } from '../api/queries'
import { enrollmentOf } from '../device/enrollment'
import { useDeviceKey } from '../device/useDeviceKey'
import { useI18n } from '../i18n'
import { useToast } from '../shared/Toasts'
import { advance, START, type EngineState } from './engine'
import { noticesFor } from './present'
import { decisionStatesOf, deviceKeysOf, teamReadings } from './readings'
import { sentDecisionIds } from './sent'

/**
 * Says in the page what is new for the viewer (docs/glossary.md and `engine.ts` say what that is): reads what the
 * screens already read, and tells the difference to the last reading. It draws nothing of its own, and what it
 * says stays in the page: there is no browser notification, so no title leaves it.
 */
export function Notices() {
  const { t } = useI18n()
  const toast = useToast()
  const api = useApi()
  const key = useDeviceKey()
  const { orgs } = useTeamStates()
  const devices = useQuery(devicesQuery(api))
  const decisions = useQuery(decisionsQuery(api))
  const path = useRouterState({ select: (state) => state.location.pathname })
  const state = useRef<EngineState>(START)

  useEffect(() => {
    if (key.status !== 'ready') return
    const own = { deviceKey: key.key.publicKey, canApprove: devices.data !== undefined && enrollmentOf(key.key.publicKey, devices.data).status === 'enrolled', sentDecisionIds: sentDecisionIds() }
    const teams = orgs.flatMap(({ teams: refs }) => refs.flatMap(({ state: one }) => (one.status === 'ready' ? [one.team] : [])))
    const next = advance(state.current, { teams: teamReadings(orgs), devices: deviceKeysOf(devices.data), decisions: decisionStatesOf(decisions.data) }, own)
    state.current = next.state
    for (const notice of noticesFor(next.events, { teams, devices: devices.data, decisions: decisions.data }, t, { onDevices: path === '/devices' })) toast(notice.message, notice.tone)
    // Only a new answer can make news; the same answers read again make none.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [orgs, devices.data, decisions.data, key.status])

  return null
}
