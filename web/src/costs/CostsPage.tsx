import { useState } from 'react'
import { useTeams } from '../api/queries'
import { teamKey } from '../api/teams'
import { failureText } from '../api/failure'
import { useI18n } from '../i18n'
import { FundingCard } from './FundingCard'
import { Loading, Skeleton } from '../shared/Skeleton'
import { TokenSection } from './TokenUsage'
import './costs.css'

export function CostsPage() {
  const { t } = useI18n()
  const { me, teams, isLoading, error } = useTeams()
  const [days, setDays] = useState<number>(14)

  if (me === undefined) {
    return <p className={error === null ? 'muted' : 'empty'} role={error === null ? undefined : 'alert'}>{error === null ? t.app.loading : failureText(t, error)}</p>
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
        <TokenSection key={teamKey(team)} team={team} days={days} onDaysChange={setDays} />
      ))}
      {isLoading ? <Loading quiet><Skeleton kind="box" /></Loading> : null}
    </>
  )
}
