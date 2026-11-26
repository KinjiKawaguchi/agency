import { QueryClient } from '@tanstack/react-query'
import { isRetryable, RETRY_DELAYS_MS, retryDelayFor } from '../api/retry'

/**
 * The client every screen reads through. A read that fails in a retryable way (see `isRetryable`) is tried
 * again after `delays`, and stays "loading" meanwhile, so a screen shows its skeleton and not a failure the
 * next try would clear. `meta.retryDelays` carries the same delays to the reads that retry on their own,
 * which the team views do so that a team that keeps failing is shown as that, in its place.
 */
export function createQueryClient(delays: readonly number[] = RETRY_DELAYS_MS): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: (failureCount, error) => failureCount < delays.length && isRetryable(error),
        retryDelay: (failureCount) => retryDelayFor(delays, failureCount),
        refetchOnWindowFocus: true,
        meta: { retryDelays: delays },
      },
    },
  })
}
