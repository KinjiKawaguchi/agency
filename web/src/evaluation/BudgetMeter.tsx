import type { BudgetPart, BudgetStatus } from '../api/clientApi'
import { useI18n } from '../i18n'

/**
 * A team's weekly budget as the node computed it: this week from Monday, what was spent, the cap, and the node's own state (`ok`, `warning`,
 * `over`). The page compares nothing: the fill's width is spent over the cap for the eye, and its color and the sentence are the state's.
 * A team whose usage could not be read has no spend and no state the page can say (the node's 0 says nothing): it is said so, with no bar.
 * The org's budget is said when the node says it is not `ok`, since an org that is over makes its teams over, and when part of it was not read.
 */
export function BudgetMeter({ status }: { status: BudgetStatus }) {
  const { t } = useI18n()
  const money = useMoney()
  const text = t.evaluation.effort.budget
  const { team, org, weekStart } = status
  const cap = team.capUsd
  // A team with no cap has no meter of its own, but its org's budget is still its own business: an org that is over makes every team over.
  if (cap === undefined) return <OrgLine org={org} />
  return (
    <div className="ev-budget">
      <p className="card-head">
        <b>{text.title}</b> <span className="muted">{team.partial === true ? text.unknown(weekStart) : text.line(money(team.spentUsd), money(cap), weekStart)}</span>
      </p>
      {team.partial === true ? (
        <p className="ev-budget-note">{text.partial}</p>
      ) : (
        <>
          <div className="ev-meter" role="img" aria-label={text.meter(money(team.spentUsd), money(cap), text.states[team.state])}>
            <span className={team.state === 'ok' ? '' : team.state} style={{ width: `${cap === 0 ? (team.state === 'over' ? 100 : 0) : Math.min(team.spentUsd / cap, 1) * 100}%` }} />
          </div>
          <p className="ev-budget-note">{text.states[team.state]}</p>
        </>
      )}
      <OrgLine org={org} />
    </div>
  )
}

/** Money as the budget writes it: a bare `$0` for nothing. */
function useMoney(): (value: number) => string {
  const { fmt } = useI18n()
  return (value) => (value === 0 ? '$0' : fmt.money(value))
}

function OrgLine({ org }: { org: BudgetPart }) {
  const { t } = useI18n()
  const money = useMoney()
  const text = t.evaluation.effort.budget
  if (org.capUsd === undefined) return null
  if (org.state === 'ok') return org.partial === true ? <p className="ev-budget-note">{text.orgPartialOk(money(org.spentUsd), money(org.capUsd))}</p> : null
  return (
    <p className="ev-budget-note">
      {text.org(org.state, money(org.spentUsd), money(org.capUsd))}
      {org.partial === true ? ` ${text.orgPartial}` : ''}
    </p>
  )
}
