import { useQueryClient } from '@tanstack/react-query'
import { failureText } from '../api/failure'
import { POLL_MS, TEAM_KEY } from '../api/queries'
import { SignInExpiredError } from '../api/session'
import { useI18n } from '../i18n'
import { useSecondsUntil } from './useCountdown'

interface Props {
  org: string
  team: string
  /** Why the team could not be read, and when the read gave up. */
  failure: { error: Error; at: number }
}

/** A team that kept failing: the reason, when the next poll tries again, and a button to try now. */
export function TeamFailed({ org, team, failure }: Props) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const seconds = useSecondsUntil(failure.at + POLL_MS)
  if (failure.error instanceof SignInExpiredError) return <p className="ov-line ov-failed" role="status">{failureText(t, failure.error)}</p>
  return (
    <div className="ov-failed-block" role="status">
      <p className="ov-line ov-failed">{t.overview.loadFailed(failure.error.message)}</p>
      <p className="ov-line ov-quiet">
        {seconds > 0 ? t.loading.retryIn(seconds) : t.loading.retrying}{' '}
        <button type="button" className="btn small" onClick={() => void queryClient.invalidateQueries({ queryKey: [...TEAM_KEY, org, team] })}>{t.loading.tryAgain}</button>
      </p>
    </div>
  )
}
