import { useEffect, useMemo, useState } from 'react'
import { useApi } from '../api/context'
import { ActivityStream, type StreamSnapshot, type Team } from './stream'

const EMPTY: StreamSnapshot = { events: [], fresh: [], failing: [], unreported: [] }

/** The activity of these teams, kept up to date while the screen is open (read every 10 seconds, after the first hour). */
export function useActivityStream(teams: readonly Team[]): StreamSnapshot {
  const api = useApi()
  const [snapshot, setSnapshot] = useState<StreamSnapshot>(EMPTY)
  const names = teams.map((team) => `${team.org}/${team.team}`).join(',')
  const stable = useMemo(() => teams, [names]) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const stream = new ActivityStream({ read: (org, team, since, signal) => api.activity(org, team, since, signal), teams: stable })
    stream.subscribe(setSnapshot)
    stream.start()
    return () => stream.stop()
  }, [api, stable])
  return snapshot
}
