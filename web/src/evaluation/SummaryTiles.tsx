import type { EvaluationSummary } from '../api/evaluationApi'
import { useI18n } from '../i18n'
import { formatDuration } from './duration'
import { allMerged } from './series'

function Tile({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <div className="ev-tile">
      <span className="ev-tile-label">{label}</span>
      <span className="ev-tile-value">{value}</span>
      <span className="ev-tile-note">{note}</span>
    </div>
  )
}

const oneDecimal = (value: number): string => String(Math.round(value * 10) / 10)

/** The period's headline numbers, as the node worked them out. New features lead: that is the speed the owner watches. */
export function SummaryTiles({ summary, days }: { summary: EvaluationSummary; days: number }) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.tiles
  const duration = (seconds: number): string => formatDuration(seconds, t.evaluation.units)
  const merged = allMerged(summary)
  const lead = (median: number | null, p90: number | null): Pick<Parameters<typeof Tile>[0], 'value' | 'note'> =>
    median === null || p90 === null ? { value: '—', note: t.evaluation.noData } : { value: duration(median), note: text.leadNote(duration(p90)) }
  return (
    <div className="ev-tiles">
      <Tile label={text.features} value={fmt.integer(summary.merged.feat)} note={text.perDay(oneDecimal(summary.merged.feat / days))} />
      <Tile label={text.merged} value={fmt.integer(merged)} note={text.perDay(oneDecimal(merged / days))} />
      <Tile label={text.prLead} {...lead(summary.prLeadMedianSeconds, summary.prLeadP90Seconds)} />
      <Tile label={text.issueLead} {...lead(summary.issueLeadMedianSeconds, summary.issueLeadP90Seconds)} />
      <Tile label={text.red} value={duration(summary.mainRedSeconds)} note={text.redNote(duration(summary.longestRedSeconds))} />
      <Tile label={text.reverts} value={fmt.integer(summary.reverts)} note={text.revertsNote(merged)} />
      <Tile label={text.cost} value={summary.costPerMergedPrUsd === null ? '—' : fmt.money(summary.costPerMergedPrUsd)} note={summary.costPerMergedPrUsd === null ? t.evaluation.noData : text.costNote} />
    </div>
  )
}
