import { QueryErrorResetBoundary, useQueryClient } from '@tanstack/react-query'
import { Component, Suspense, useEffect, useState, type ErrorInfo, type ReactNode } from 'react'
import { failureText } from '../api/failure'
import { POLL_MS } from '../api/queries'
import { SignInExpiredError } from '../api/session'
import { useI18n } from '../i18n'
import { useSecondsUntil } from './useCountdown'

const hasFailed = (query: { state: { status: string } }): boolean => query.state.status === 'error'

/**
 * The region went wrong for good: what, why, when it tries again by itself, and a button to try now. The
 * automatic try is made quietly: the failure stays on screen ("Trying again…") while the queries that failed
 * are read again, and the region is shown again only once they have passed, so that it does not flash
 * between a skeleton and the failure every half minute. If they fail again the countdown starts over.
 */
function RegionFailed({ what, error, retry }: { what: string; error: Error; retry: () => void }) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [failedAt, setFailedAt] = useState(() => Date.now())
  const [isTrying, setIsTrying] = useState(false)
  const seconds = useSecondsUntil(failedAt + POLL_MS)
  useEffect(() => {
    if (seconds > 0 || isTrying) return
    setIsTrying(true)
    void queryClient.refetchQueries({ predicate: hasFailed }).then(() => {
      if (queryClient.getQueryCache().findAll({ predicate: hasFailed }).length === 0) retry()
      else {
        setFailedAt(Date.now())
        setIsTrying(false)
      }
    })
  }, [seconds, isTrying, queryClient, retry])
  return (
    <div className="region-failed" role="alert">
      <p>
        {error instanceof SignInExpiredError ? failureText(t, error) : t.loading.failed(what, error.message)}
        <span className="muted">{isTrying ? t.loading.retrying : t.loading.retryIn(seconds)}</span>
      </p>
      <button type="button" className="btn small" onClick={retry}>{t.loading.tryAgain}</button>
    </div>
  )
}

interface BoundaryProps {
  children: ReactNode
  /** Clears what the queries remember of their failure, so that trying again asks again. */
  onReset: () => void
  failed: (error: Error, retry: () => void) => ReactNode
}

class Boundary extends Component<BoundaryProps, { error: Error | null }> {
  override state: { error: Error | null } = { error: null }

  static getDerivedStateFromError(error: Error): { error: Error } {
    return { error }
  }

  override componentDidCatch(_error: Error, _info: ErrorInfo): void {
    // The failure is shown by `failed`; nothing else is done with it.
  }

  private readonly retry = (): void => {
    this.props.onReset()
    this.setState({ error: null })
  }

  override render(): ReactNode {
    return this.state.error === null ? this.props.children : this.props.failed(this.state.error, this.retry)
  }
}

interface RegionProps {
  /** What the region shows, for its failure message ("the overview"). */
  what: string
  /** The shape of the region while it loads. */
  skeleton: ReactNode
  children: ReactNode
}

/**
 * A part of a screen that reads on its own: while it reads, its skeleton stands in; when the read fails for good,
 * only this part says so, with the reason and when it tries again, and a button to try now. The rest of the
 * page is never blanked by it.
 */
export function Region({ what, skeleton, children }: RegionProps) {
  return (
    <QueryErrorResetBoundary>
      {({ reset }) => (
        <Boundary onReset={reset} failed={(error, retry) => <RegionFailed what={what} error={error} retry={retry} />}>
          <Suspense fallback={skeleton}>{children}</Suspense>
        </Boundary>
      )}
    </QueryErrorResetBoundary>
  )
}
