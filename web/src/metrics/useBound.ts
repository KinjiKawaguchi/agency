import { useI18n } from '../i18n'
import type { Bound } from './latency'

/** How a bucket's upper bound is written: "≤ 100 ms", "≤ 2.5 s", or "slower than 30 s" for what fell beyond the last bound. */
export function useBoundText(): { duration: (ms: number) => string; bound: (value: Bound) => string } {
  const { t, fmt } = useI18n()
  const text = t.metrics
  const duration = (ms: number): string => (ms >= 1000 ? text.seconds(String(ms / 1000)) : text.ms(fmt.integer(ms)))
  return {
    duration,
    // The last bound of the node's buckets is the one a "beyond" is slower than.
    bound: (value) => (value.kind === 'atMost' ? text.atMost(duration(value.ms)) : text.slowerThan(duration(LAST_BOUND_MS))),
  }
}

const LAST_BOUND_MS = 30_000
