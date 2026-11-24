import { useState } from 'react'
import { useTeams } from '../api/queries'
import { teamKey } from '../api/teams'
import { useI18n } from '../i18n'
import { FundingCard } from './FundingCard'
import { TokenUsage } from './TokenUsage'
import './costs.css'

const PERIODS = [7, 14, 30] as const

export function CostsPage() {
  const { t } = useI18n()
  const { me, teams, error } = useTeams()
  const [days, setDays] = useState<number>(14)

  if (me === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : t.app.unreachable(error.message)}</p>
  }
  const now = Date.now()
  return (
    <>
      <h1 className="page-title">{t.costs.title}</h1>
      <h2 className="sub-head">{t.costs.funding}</h2>
      {me.funding.length === 0 ? (
        <p className="empty">{t.costs.noFunding}</p>
      ) : (
        <div className="funds">
          {me.funding.map((source) => (
            <FundingCard key={source.id} source={source} now={now} />
          ))}
        </div>
      )}
      {teams.map((team) => (
        <section key={teamKey(team)}>
          <h2 className="sub-head">{t.costs.tokens.title(team.view.label)}</h2>
          <div className="period" role="group" aria-label={t.costs.tokens.period}>
            {PERIODS.map((period) => (
              <button key={period} className={`btn small${period === days ? ' primary' : ''}`} type="button" aria-pressed={period === days} onClick={() => setDays(period)}>
                {t.costs.tokens.days(period)}
              </button>
            ))}
            <span className="muted">{t.costs.tokens.estimate}</span>
          </div>
          <TokenUsage view={team.view} days={days} />
        </section>
      ))}
    </>
  )
}
