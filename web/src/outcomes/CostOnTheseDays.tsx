import { useQuery } from '@tanstack/react-query'
import { useApi } from '../api/context'
import { evaluationQuery } from '../api/queries'
import { Columns } from '../evaluation/Columns'
import { niceMax } from '../evaluation/series'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'
import { costByDay } from './costByDay'

interface Props {
  org: string
  /** The days the KPI above is drawn over: the cost is on the same ones, under the same columns. */
  dates: readonly string[]
}

/** What the org's work cost, API-priced, on the days a KPI is drawn over, so that a movement can be read next to the effort of its days. */
export function CostOnTheseDays({ org, dates }: Props) {
  const { t, fmt } = useI18n()
  const text = t.outcomes.cost
  const api = useApi()
  const evaluation = useQuery(evaluationQuery(api, org, dates.length))
  if (evaluation.data === undefined) {
    return evaluation.error === null ? <Loading quiet><Skeleton kind="box" /></Loading> : <p className="muted oc-cost-note">{text.unavailable}</p>
  }
  const { days, teamsWithout } = costByDay(evaluation.data, dates)
  const known = days.filter((day) => day.usd !== undefined)
  if (known.length === 0) {
    return (
      <>
        <p className="muted oc-cost-note">{text.none}</p>
        {teamsWithout === 0 ? null : <p className="muted oc-cost-note">{text.partial(teamsWithout)}</p>}
      </>
    )
  }
  const max = niceMax(Math.max(...known.map((day) => day.usd ?? 0))) /* stand-in-ok(scale): an axis maximum from the days that have a cost */
  return (
    <div className="oc-cost">
      <p className="muted oc-cost-note">{text.title}</p>
      <Columns
        label={text.title}
        max={max}
        format={fmt.money}
        describe={(date, total) => text.day(date, fmt.money(total))}
        noData={text.noCost}
        columns={days.map((day) => (day.usd === undefined ? { date: day.date, isMissing: true, segments: [] } : { date: day.date, segments: [{ key: 'cost', label: text.series, color: 'var(--series-2)', value: day.usd }] }))}
      />
      {teamsWithout === 0 ? null : <p className="muted oc-cost-note">{text.partial(teamsWithout)}</p>}
    </div>
  )
}
