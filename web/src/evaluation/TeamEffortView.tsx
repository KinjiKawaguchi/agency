import type { TeamEffort, TeamPlan } from '../api/evaluationApi'
import { useI18n } from '../i18n'
import { AllocationRows } from './AllocationRows'
import { BudgetMeter } from './BudgetMeter'
import { Columns } from './Columns'
import { EFFORT_COLORS, EFFORT_TYPES, pointsFromPlan, sharesOf, shownShare, totalCost, weekSpend } from './effort'
import { daysEnding, lastDayOf, niceMax } from './series'

export type PlanState = { kind: 'stated'; plan: TeamPlan } | { kind: 'none' } | { kind: 'unreadable' }

interface Props {
  label: string
  effort: TeamEffort
  plan: PlanState
  days: number
  generatedAt: string
  until: string | undefined
}

/** One team's effort: its cost by work type against the plan, its weekly budget, and its cost by day. */
export function TeamEffortView({ label, effort, plan, days, generatedAt, until }: Props) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.effort
  const stated = plan.kind === 'stated' ? plan.plan : undefined
  const last = lastDayOf(effort.byDay, until, generatedAt)
  const dates = daysEnding(last, days)
  const byDate = new Map(effort.byDay.map((day) => [day.date, day]))
  const columns = dates.map((date) => ({ date, segments: EFFORT_TYPES.map((type) => ({ key: type, label: text.types[type], color: EFFORT_COLORS[type], value: byDate.get(date)?.costUsd[type] ?? 0 })) }))
  const money = (value: number): string => (value === 0 ? '$0' : fmt.money(value))
  const max = niceMax(Math.max(0, ...columns.map((column) => column.segments.reduce((sum, segment) => sum + segment.value, 0))))
  const shares = sharesOf(effort)
  return (
    <section className="ev-team">
      <h3 className="sub-head">{t.evaluation.team(label)}</h3>
      <div className="card">
        <p className="card-head">
          <b>{text.allocation.title}</b> <span className="muted">{stated === undefined ? '' : text.allocation.hint}</span>
        </p>
        {plan.kind === 'none' ? <p className="muted ev-plan-note">{text.noPlan}</p> : null}
        {plan.kind === 'unreadable' ? <p className="muted ev-plan-note">{text.planUnreadable}</p> : null}
        <AllocationRows effort={effort} plan={stated} />
        <p className="muted ev-plan-note">{text.allocation.unclassifiedNote}</p>
        {stated === undefined ? null : <BudgetMeter spent={weekSpend(effort.byDay, last)} budget={stated.weeklyBudgetUsd} />}
      </div>
      <div className="card">
        <p className="card-head">
          <b>{text.byDay}</b> <span className="muted">{text.byDayHint}</span>
        </p>
        <ul className="legend">
          {EFFORT_TYPES.map((type) => (
            <li key={type}>
              <i style={{ background: EFFORT_COLORS[type] }} />
              {text.types[type]}
            </li>
          ))}
        </ul>
        <Columns label={text.byDay} max={max} format={money} hasTotal describe={(date, total) => `${date}: ${money(total)}`} columns={columns} />
      </div>
      <details className="ev-details">
        <summary>{text.table.show}</summary>
        <div className="ev-table-wrap">
          <table>
            <caption className="sr-only">{text.table.caption(label)}</caption>
            <thead>
              <tr>
                <th scope="col">{text.table.type}</th>
                <th scope="col" className="num">{text.table.cost}</th>
                <th scope="col" className="num">{text.table.share}</th>
                <th scope="col" className="num">{text.table.intended}</th>
              </tr>
            </thead>
            <tbody>
              {EFFORT_TYPES.map((type) => {
                const intended = stated !== undefined && type !== 'unclassified' ? stated.allocation[type] : undefined
                return (
                  <tr key={type}>
                    <th scope="row">{text.types[type]}</th>
                    <td className="num">{money(effort.byType[type].costUsd)}</td>
                    <td className="num">{fmt.percent(shownShare(shares[type]))}</td>
                    <td className="num">{intended === undefined ? '—' : `${fmt.percent(intended)} (${pointsFromPlan(shares[type], intended) > 0 ? '+' : ''}${pointsFromPlan(shares[type], intended)})`}</td>
                  </tr>
                )
              })}
              <tr>
                <th scope="row">{text.table.total}</th>
                <td className="num">{money(totalCost(effort))}</td>
                <td className="num" />
                <td className="num" />
              </tr>
            </tbody>
          </table>
        </div>
      </details>
    </section>
  )
}
