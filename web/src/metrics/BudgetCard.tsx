import type { LatencySeries } from '../api/metricsApi'
import { useI18n } from '../i18n'
import { inNodeReads, readsOverBudget, verdictOf } from './latency'
import { useBoundText } from './useBound'

/** In-node reads against the node's budget: one verdict, and the reads that break it. Drawn only when the node names a budget. */
export function BudgetCard({ series, budgetMs }: { series: readonly LatencySeries[]; budgetMs: number }) {
  const { t, fmt } = useI18n()
  const text = t.metrics.budget
  const { duration, bound } = useBoundText()
  const reads = inNodeReads(series)
  const verdict = reads === undefined ? undefined : verdictOf(reads.p95, budgetMs)
  const over = readsOverBudget(series, budgetMs)
  return (
    <div className="card mt-card" aria-labelledby="mt-budget">
      <h3 className="sub-head" id="mt-budget">{text.title}</h3>
      <p className="muted">{text.lead(duration(budgetMs))}</p>
      {reads === undefined || reads.p95 === undefined || verdict === undefined ? (
        <p>{text.noReads}</p>
      ) : (
        <p className={`mt-verdict is-${verdict.kind}`}>
          <strong>{verdict.kind === 'within' ? text.within(bound(reads.p95)) : verdict.kind === 'over' ? text.over(bound(reads.p95)) : text.undetermined(bound(reads.p95), duration(budgetMs))}</strong> {text.requests(fmt.integer(reads.count))}
        </p>
      )}
      {over.length === 0 ? null : (
        <>
          <h4 className="mt-sub">{text.overRoutes}</h4>
          <ul className="mt-routes">
            {over.map((row) => (
              <li key={row.key}>
                <code>{row.key}</code> {row.p95 === undefined ? null : <span>{bound(row.p95)}</span>}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  )
}
