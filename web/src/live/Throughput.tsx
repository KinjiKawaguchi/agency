import { useQueries } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { useApi } from '../api/context'
import { evaluationQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { allMerged } from '../evaluation/series'
import { formatDuration } from '../evaluation/duration'
import { Skeleton } from '../shared/Skeleton'
import type { TaggedEvent } from './stream'
import { binsOf, countsOf, medianOf } from './merges'

/** How long a tile stays lit after a merge. */
const LANDED_MS = 2_600

interface Props {
  events: readonly TaggedEvent[]
  fresh: readonly TaggedEvent[]
  /** The orgs whose evaluation gives the lead time and today's merges. */
  orgs: readonly string[]
  now: number
  /** Some team's feed has not been read yet: what is counted from the events is not the whole. */
  isReading: boolean
}

/** Merges per hour as they land, the lead time, and the day's merges; a tile lights up, and says what landed, when a merge arrives. */
export function Throughput({ events, fresh, orgs, now, isReading }: Props) {
  const { t } = useI18n()
  const api = useApi()
  const evaluations = useQueries({ queries: orgs.map((org) => ({ ...evaluationQuery(api, org, 14), refetchInterval: 60_000 })) })
  const isEvaluating = evaluations.some((one) => one.isPending)
  const hasEvaluationFailed = evaluations.some((one) => one.isError)
  const teams = evaluations.flatMap((one) => (one.data === undefined ? [] : one.data.teams))
  const lead = medianOf(teams.flatMap((team) => (team.summary.prLeadMedianSeconds === undefined ? [] : [team.summary.prLeadMedianSeconds])))
  const evaluationToday = teams.length === 0 ? undefined : teams.reduce((sum, team) => sum + (team.byDay.find((day) => day.date === team.until) === undefined ? 0 : allMerged(team.byDay.find((day) => day.date === team.until)!)), 0)
  const bins = binsOf(events, now)
  // Both counts come from the same events, so that the day's is never below the hour's.
  const counts = countsOf(events, now, evaluationToday)
  const peak = Math.max(1, ...bins)

  const [landed, setLanded] = useState<string | undefined>(undefined)
  useEffect(() => {
    const merge = fresh.find(({ event }) => event.kind === 'prMerged')
    if (merge === undefined || merge.event.kind !== 'prMerged') return undefined
    setLanded(merge.event.ref)
    const timer = window.setTimeout(() => setLanded(undefined), LANDED_MS)
    return () => window.clearTimeout(timer)
  }, [fresh])

  return (
    <div className={`lv-tiles${landed === undefined ? '' : ' is-landed'}`}>
      <div className="lv-tile">
        <span className="lv-tile-label">{t.live.throughput.lastHour}</span>
        <span className="lv-tile-value">{isReading ? <Skeleton kind="short" /> : counts.lastHour}</span>
        {isReading ? (
          <Skeleton kind="box" />
        ) : (
          <div className="lv-bins" role="img" aria-label={t.live.throughput.bins}>
            {bins.map((count, index) => (
              <span key={index} className="lv-bin" style={{ height: `${(count / peak) * 100}%` }} title={String(count)} />
            ))}
          </div>
        )}
        <span className="lv-burst" aria-hidden="true" />
      </div>
      <div className="lv-tile">
        <span className="lv-tile-label">{t.live.throughput.today}</span>
        <span className="lv-tile-value">{isReading || isEvaluating ? <Skeleton kind="short" /> : hasEvaluationFailed ? t.live.throughput.none : counts.today}</span>
      </div>
      <div className="lv-tile">
        <span className="lv-tile-label">{t.live.throughput.lead}</span>
        <span className="lv-tile-value">{isEvaluating ? <Skeleton kind="short" /> : lead === undefined ? t.live.throughput.none : formatDuration(lead, t.evaluation.units)}</span>
      </div>
      <p className="lv-landed" role="status">{landed === undefined ? '' : t.live.throughput.landed(landed)}</p>
    </div>
  )
}
