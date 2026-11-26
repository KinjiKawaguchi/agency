import { ApiError } from './client'
import { isRetryable, RETRY_DELAYS_MS, retryDelayFor, withRetry } from './retry'
import { SignInExpiredError } from './session'

describe('isRetryable', () => {
  it.each([500, 502, 503, 504, 408, 429])('retries an answer of %i', (status) => {
    expect(isRetryable(new ApiError('later', status))).toBe(true)
  })

  it.each([400, 401, 403, 404, 409, 422])('does not retry an answer of %i, which says the same again', (status) => {
    expect(isRetryable(new ApiError('no', status))).toBe(false)
  })

  it('retries a call that got no answer, and not an expired sign-in or anything else', () => {
    expect(isRetryable(new TypeError('Failed to fetch'))).toBe(true)
    expect(isRetryable(new SignInExpiredError())).toBe(false)
    expect(isRetryable(new Error('bug'))).toBe(false)
    expect(isRetryable('text')).toBe(false)
  })
})

describe('retryDelayFor', () => {
  it('backs off 1, 2, 4, 8, 15 seconds and repeats the last', () => {
    expect(RETRY_DELAYS_MS.map((_, attempt) => retryDelayFor(RETRY_DELAYS_MS, attempt))).toEqual([1000, 2000, 4000, 8000, 15000])
    expect(retryDelayFor(RETRY_DELAYS_MS, 9)).toBe(15000)
    expect(retryDelayFor([], 0)).toBe(0)
  })
})

describe('withRetry', () => {
  const busy = () => new ApiError('The board has not been read yet.', 503)

  it('answers what the read answers once it passes, after the retries it needed', async () => {
    let calls = 0
    const read = () => (++calls < 3 ? Promise.reject(busy()) : Promise.resolve('ready'))
    await expect(withRetry(read, [0, 0, 0])).resolves.toBe('ready')
    expect(calls).toBe(3)
  })

  it('gives up with the last error when the delays are used up', async () => {
    let calls = 0
    const read = () => Promise.reject(Object.assign(busy(), { attempt: ++calls }))
    await expect(withRetry(read, [0, 0])).rejects.toMatchObject({ status: 503, attempt: 3 })
  })

  it('throws a final error at once, without waiting', async () => {
    let calls = 0
    await expect(withRetry(() => (++calls, Promise.reject(new ApiError('gone', 404))), [0, 0, 0])).rejects.toMatchObject({ status: 404 })
    expect(calls).toBe(1)
  })

  it('waits the delays between tries, and stops when the signal aborts', async () => {
    vi.useFakeTimers()
    try {
      const controller = new AbortController()
      let calls = 0
      const result = withRetry(() => (++calls, Promise.reject(busy())), [1000, 2000], controller.signal)
      const settled = result.catch((error: unknown) => error)
      await vi.advanceTimersByTimeAsync(999)
      expect(calls).toBe(1)
      await vi.advanceTimersByTimeAsync(1)
      expect(calls).toBe(2)
      controller.abort(new DOMException('aborted', 'AbortError'))
      await vi.advanceTimersByTimeAsync(0)
      expect(await settled).toMatchObject({ name: 'AbortError' })
      expect(calls).toBe(2)
    } finally {
      vi.useRealTimers()
    }
  })
})
