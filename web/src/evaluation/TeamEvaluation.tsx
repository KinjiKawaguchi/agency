import type { TeamEvaluation as TeamNumbers } from '../api/evaluationApi'
import { useI18n } from '../i18n'
import { Columns } from './Columns'
import { DayTable } from './DayTable'
import { durationAxis, formatDuration } from './duration'
import { countMax, fillDays, lastDayOf, niceMax, PULL_REQUEST_TYPES, rollingCostPerPullRequest, rollingMedian, allMerged, REVERT_COLOR, TYPE_COLORS } from './series'
import { SummaryTiles } from './SummaryTiles'
import { HOUR_SECONDS } from './time'
import { TrendChart } from './TrendChart'

const TREND_DAYS = 7

interface Props {
  label: string
  numbers: TeamNumbers
  days: number
  generatedAt: string
  /** The period's last day, when the node names it. */
  until: string | undefined
}

/** One team's evaluation: the headline numbers, then each of them over the days. */
export function TeamEvaluation({ label, numbers, days, generatedAt, until }: Props) {
  const { t, fmt } = useI18n()
  const text = t.evaluation
  const rows = fillDays(numbers.byDay, days, lastDayOf(numbers.byDay, until, generatedAt))
  const dates = rows.map((row) => row.date)
  const duration = (seconds: number): string => formatDuration(seconds, text.units)

  const mergedMax = countMax(Math.max(0, ...rows.map((row) => allMerged(row))))
  const prLead = rollingMedian(rows, (row) => row.prLeadSeconds, TREND_DAYS)
  const issueLead = rollingMedian(rows, (row) => row.issueLeadSeconds, TREND_DAYS)
  const leadAxis = durationAxis(Math.max(HOUR_SECONDS, ...prLead.map((value) => value ?? 0), ...issueLead.map((value) => value ?? 0)), text.units)
  const redAxis = durationAxis(Math.max(HOUR_SECONDS, ...rows.map((row) => row.mainRedSeconds)), text.units)
  const revertsMax = countMax(Math.max(0, ...rows.map((row) => row.reverts)))
  const costPerPr = rollingCostPerPullRequest(rows, TREND_DAYS)
  const costMax = niceMax(Math.max(0, ...costPerPr.map((value) => value ?? 0)))
  const money = (value: number): string => (value === 0 ? '$0' : fmt.money(value))

  return (
    <section className="ev-team">
      <h2 className="sub-head">{text.team(label)}</h2>
      <SummaryTiles summary={numbers.summary} days={days} />

      <div className="card">
        <p className="card-head">
          <b>{text.charts.merged}</b> <span className="muted">{text.charts.mergedHint}</span>
        </p>
        <ul className="legend">
          {PULL_REQUEST_TYPES.map((type) => (
            <li key={type}>
              <i style={{ background: TYPE_COLORS[type] }} />
              {text.types[type]}
            </li>
          ))}
          <li>
            <i style={{ background: REVERT_COLOR }} />
            {text.types.revert}
          </li>
        </ul>
        <Columns
          label={text.charts.merged}
          max={mergedMax}
          format={fmt.integer}
          hasTotal
          describe={text.charts.dayMerged}
          columns={rows.map((row) => ({ date: row.date, segments: [...PULL_REQUEST_TYPES.map((type) => ({ key: type, label: text.types[type], color: TYPE_COLORS[type], value: row.merged[type] })), { key: 'revert', label: text.types.revert, color: REVERT_COLOR, value: row.reverts }] }))}
        />
      </div>

      <div className="ev-pair">
        <div className="card">
          <p className="card-head">
            <b>{text.charts.lead}</b> <span className="muted">{text.charts.leadHint}</span>
          </p>
          <ul className="legend">
            <li><i style={{ background: 'var(--series-1)' }} />{text.charts.prLead}</li>
            <li><i style={{ background: 'var(--series-2)' }} />{text.charts.issueLead}</li>
          </ul>
          <TrendChart
            dates={dates}
            max={leadAxis.max}
            format={leadAxis.format}
            label={text.charts.lead}
            noData={text.charts.noSamples}
            lines={[
              { key: 'pr', label: text.charts.prLead, color: 'var(--series-1)', values: prLead },
              { key: 'issue', label: text.charts.issueLead, color: 'var(--series-2)', values: issueLead },
            ]}
          />
        </div>
        <div className="card">
          <p className="card-head">
            <b>{text.charts.cost}</b> <span className="muted">{text.charts.costHint}</span>
          </p>
          <TrendChart dates={dates} max={costMax} format={money} label={text.charts.cost} noData={text.charts.noSamples} lines={[{ key: 'cost', label: text.charts.cost, color: 'var(--series-1)', values: costPerPr }]} />
        </div>
      </div>

      <div className="ev-pair">
        <div className="card">
          <p className="card-head">
            <b>{text.charts.red}</b>
          </p>
          <Columns
            label={text.charts.red}
            max={redAxis.max}
            format={redAxis.format}
            describe={(date, total) => `${date}: ${duration(total)}`}
            columns={rows.map((row) => ({ date: row.date, segments: [{ key: 'red', label: text.tiles.red, color: 'var(--series-1)', value: row.mainRedSeconds }] }))}
          />
        </div>
        <div className="card">
          <p className="card-head">
            <b>{text.charts.reverts}</b>
          </p>
          <Columns
            label={text.charts.reverts}
            max={revertsMax}
            format={fmt.integer}
            describe={(date, total) => `${date}: ${fmt.integer(total)}`}
            columns={rows.map((row) => ({ date: row.date, segments: [{ key: 'reverts', label: text.tiles.reverts, color: 'var(--series-1)', value: row.reverts }] }))}
          />
        </div>
      </div>

      <DayTable team={label} rows={rows} />
    </section>
  )
}
