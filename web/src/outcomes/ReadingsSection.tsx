import type { KpiInfo, Reading, Readings } from '../api/outcomesApi'
import { useI18n } from '../i18n'
import { formatKpiChange, formatKpiValue } from './format'
import { movementOf } from './movement'
import { refusalOf } from './refusal'

interface RowProps {
  reading: Reading
  kpi: KpiInfo | undefined
  /** The titles of the epics that overlap, when the answer has them; the reference otherwise. */
  titleOf: (epic: string) => string
}

/** One aim of a team's plan: why it can or cannot be read, and, when it can, the two numbers and how they moved. Never "because". */
function ReadingRow({ reading, kpi, titleOf }: RowProps) {
  const { t, fmt, locale } = useI18n()
  const text = t.outcomes.readings
  const show = (value: number): string => (kpi === undefined ? String(value) : formatKpiValue(value, kpi, locale, t.evaluation.units))
  const { before, after, change } = reading
  const refusal = refusalOf(reading, text.states, fmt.shortDate)
  const movement = before === undefined || after === undefined ? undefined : movementOf(before.value, after.value, kpi?.goodWhen)
  return (
    <li className="oc-reading">
      <h4>{reading.epicTitle ?? reading.epic}</h4>
      <p className="muted">
        {text.aimedAt(kpi?.label ?? reading.kpi)}
        {reading.shippedOn === undefined ? '' : ` · ${text.shipped(fmt.shortDate(reading.shippedOn))}`}
      </p>
      {refusal === undefined ? null : <p className="oc-refusal">{refusal}</p>}
      {reading.state !== 'ready' || before === undefined || after === undefined ? null : (
        <>
          <p>{text.ready(show(before.value), fmt.shortDate(before.date), show(after.value), fmt.shortDate(after.date))}</p>
          {change === undefined || kpi === undefined ? null : (
            <p className={`oc-move${movement?.quality === undefined ? '' : ` is-${movement.quality}`}`}>
              {movement === undefined ? null : `${t.outcomes.movement.lead(t.outcomes.movement[movement.direction])} `}
              {formatKpiChange(change, kpi, locale, t.evaluation.units)}
              {reading.changeRatio === undefined ? '' : ` (${reading.changeRatio > 0 ? '+' : reading.changeRatio < 0 ? '−' : ''}${fmt.percent(Math.abs(reading.changeRatio) * 100)})`}
              {movement?.quality === undefined ? null : <b className="oc-quality">{t.outcomes.movement[movement.quality]}</b>}
            </p>
          )}
          <p className="muted">{text.correlation}</p>
          {reading.overlapping.length === 0 ? null : <p className="oc-overlap">{text.overlap(reading.overlapping.map(titleOf).join(', '))}</p>}
        </>
      )}
    </li>
  )
}

interface SectionProps {
  readings: Readings
  kpis: readonly KpiInfo[]
  teamLabels: ReadonlyMap<string, string>
}

/** The readings of an org, by team, in the order of the plan. An org with no aims says so. */
export function ReadingsSection({ readings, kpis, teamLabels }: SectionProps) {
  const { t } = useI18n()
  const text = t.outcomes.readings
  const titles = new Map(readings.readings.map((one) => [one.epic, one.epicTitle ?? one.epic]))
  const teams = [...new Set(readings.readings.map((one) => one.team))]
  return (
    <section className="oc-readings">
      <h2 className="sub-head">{text.title}</h2>
      <p className="muted ev-lead">{text.lead}</p>
      {teams.length === 0 ? <p className="empty">{text.none}</p> : null}
      {teams.map((team) => (
        <section key={team} className="oc-team">
          <h3 className="sub-head">{text.team(teamLabels.get(team) ?? team)}</h3>
          <ul className="oc-reading-list">
            {readings.readings
              .filter((one) => one.team === team)
              .map((one) => (
                <ReadingRow key={`${one.epic}/${one.kpi}`} reading={one} kpi={kpis.find((kpi) => kpi.id === one.kpi)} titleOf={(epic) => titles.get(epic) ?? epic} />
              ))}
          </ul>
        </section>
      ))}
    </section>
  )
}
