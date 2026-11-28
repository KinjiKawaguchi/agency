import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { ApiError } from '../api/client'
import { failureText } from '../api/failure'
import { aboutQuery, budgetQuery } from '../api/queries'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { BudgetMeter } from './BudgetMeter'

/**
 * A team's weekly budget. A node that lists the `budget` capability says it: the meter shows that answer. A node that does not is not asked,
 * and the page says what the plan states (the weekly budget) and that the node does not report the week's spend; it computes nothing itself.
 */
export function TeamBudget({ org, team, planned }: { org: string; team: string; planned: number | undefined }) {
  const { t, fmt } = useI18n()
  const text = t.evaluation.effort.budget
  const api = useApi()
  const about = useQuery(aboutQuery(api))
  const hasBudget = about.data?.capabilities.includes('budget') === true
  const budget = useQuery({ ...budgetQuery(api, org, team), enabled: hasBudget })
  const plain = planned === undefined ? null : <p className="muted ev-plan-note">{text.plain(fmt.money(planned))}</p>
  if (about.data === undefined) {
    return about.error === null ? <Loading quiet><Skeleton kind="short" /></Loading> : <p className="empty" role="alert">{failureText(t, about.error)}</p>
  }
  if (!hasBudget) return plain
  if (budget.data !== undefined) return <BudgetMeter status={budget.data} />
  if (budget.error === null) return <Loading quiet><Skeleton kind="short" /></Loading>
  if (budget.error instanceof ApiError && budget.error.status === 404) return plain
  return <p className="empty" role="alert">{failureText(t, budget.error)}</p>
}
