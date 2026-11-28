import { useEffect } from 'react'
import { useApi } from '../api/context'
import { useStanding } from '../guest/useStanding'
import type { Telemetry } from './telemetry'

/** Reports how long the page's calls took, for the owner: a hosted member's page forgets them, and the page waits to know which it is. */
export function TelemetryReporting({ telemetry }: { telemetry: Telemetry }): null {
  const api = useApi()
  const { status } = useStanding()
  useEffect(() => {
    if (status === 'guest') telemetry.forbid()
    if (status === 'member') return telemetry.start(api.reportTelemetry)
    return undefined
  }, [api, status, telemetry])
  return null
}
