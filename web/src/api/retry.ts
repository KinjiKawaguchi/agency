import { ApiError } from './client'
import { SignInExpiredError } from './session'

/**
 * How long to wait before each retry of a read that failed in a way that may pass: five retries, about half
 * a minute in all. After them the failure is shown, and the next poll (30 s later) tries again.
 */
export const RETRY_DELAYS_MS: readonly number[] = [1_000, 2_000, 4_000, 8_000, 15_000]

/**
 * Whether a failed read is worth retrying. The node answers 503 while a team's view is not ready yet (a board
 * read has just started) or its runtime timed out under load; a call that got no answer at all (offline, a
 * timeout) is a `TypeError` from `fetch`. Every other answer below 500 is final: the request is wrong, the
 * thing is gone, or the viewer is not allowed, and asking again says the same. An expired sign-in is final too.
 */
export function isRetryable(error: unknown): boolean {
  if (error instanceof SignInExpiredError) return false
  if (error instanceof ApiError) return error.status >= 500 || error.status === 408 || error.status === 429
  return error instanceof TypeError
}

/** The wait before retry number `attempt` (0 is the first retry); the last delay repeats if there are more. */
export const retryDelayFor = (delays: readonly number[], attempt: number): number => delays[Math.min(attempt, delays.length - 1)] ?? 0

function sleep(ms: number, signal: AbortSignal | undefined): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal?.aborted === true) return reject(signal.reason)
    const timer = setTimeout(resolve, ms)
    signal?.addEventListener('abort', () => {
      clearTimeout(timer)
      reject(signal.reason)
    }, { once: true })
  })
}

/**
 * Runs a read, and again after each delay while it fails in a retryable way. Throws the last error once the
 * delays are used up, at once for a final one, and as soon as the signal aborts.
 */
export async function withRetry<T>(read: () => Promise<T>, delays: readonly number[], signal?: AbortSignal): Promise<T> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await read()
    } catch (error) {
      if (!isRetryable(error) || attempt >= delays.length) throw error
      await sleep(retryDelayFor(delays, attempt), signal)
    }
  }
}
