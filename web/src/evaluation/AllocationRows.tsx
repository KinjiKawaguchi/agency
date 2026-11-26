import type { TeamEffort, TeamPlan } from '../api/evaluationApi'
import { useI18n } from '../i18n'
import { EFFORT_COLORS, EFFORT_TYPES, pointsFromPlan, sharesOf, shownShare, WORK_TYPES } from './effort'

/**
 * One row for each work type: its share of the period's cost as a bar on a 0 to 100% track, and the
 * org's intended share as a mark on the same track, so how far off a type is reads as the gap between them.
 */
export function AllocationRows({ effort, plan }: { effort: TeamEffort; plan: TeamPlan | undefined }) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.effort
  const shares = sharesOf(effort)
  return (
    <ul className="ev-alloc">
      {EFFORT_TYPES.map((type) => {
        const share = shares[type]
        const intended = plan !== undefined && (WORK_TYPES as readonly string[]).includes(type) ? plan.allocation[type as (typeof WORK_TYPES)[number]] : undefined
        const points = intended === undefined ? 0 : pointsFromPlan(share, intended)
        return (
          <li key={type} className="ev-alloc-row">
            <span className="ev-alloc-name">
              <i style={{ background: EFFORT_COLORS[type] }} />
              {text.types[type]}
            </span>
            <span className="ev-alloc-track" aria-hidden="true">
              <span className="ev-alloc-fill" style={{ width: `${Math.min(share, 100)}%`, background: EFFORT_COLORS[type] }} />
              {intended === undefined ? null : <span className="ev-alloc-mark" style={{ left: `calc(${intended}% - ${intended / 50}px)` }} />}
            </span>
            <span className="ev-alloc-figures">
              <b>{text.allocation.actual(fmt.percent(shownShare(share)))}</b>
              {intended === undefined ? null : (
                <span className="muted">
                  {text.allocation.intended(fmt.percent(intended))}, {points === 0 ? text.allocation.onPlan : points > 0 ? text.allocation.above(points) : text.allocation.below(-points)}
                </span>
              )}
            </span>
          </li>
        )
      })}
    </ul>
  )
}
