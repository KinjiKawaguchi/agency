import { useI18n } from '../i18n'

/** The last seven days' spend against the weekly budget the org set. Over the budget the fill turns to the seal color and the text says by how much. */
export function BudgetMeter({ spent, budget }: { spent: number; budget: number }) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.effort.budget
  const money = (value: number): string => (value === 0 ? '$0' : fmt.money(value))
  const isOver = spent > budget
  return (
    <div className="ev-budget">
      <p className="card-head">
        <b>{text.title}</b> <span className="muted">{text.line(money(spent), money(budget))}</span>
      </p>
      <div className="ev-meter" role="img" aria-label={text.meter(money(spent), money(budget))}>
        <span className={isOver ? 'over' : ''} style={{ width: `${budget === 0 ? 100 : Math.min(spent / budget, 1) * 100}%` }} />
      </div>
      <p className="ev-budget-note">{isOver ? text.over(money(spent - budget)) : text.left(money(budget - spent))}</p>
    </div>
  )
}
