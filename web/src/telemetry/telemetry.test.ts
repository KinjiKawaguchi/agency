import { REPORT_EVERY_MS } from './reporter'
import { createTelemetry } from './telemetry'

const known = { route: 'GET /api/me', status: '2xx', ms: 12 } as const

/** A report that is taken, and the reports it was given. */
function taken() {
  const reports: unknown[][] = []
  return { reports, report: async (samples: readonly unknown[]) => void reports.push([...samples]) }
}

afterEach(() => {
  vi.useRealTimers()
})

describe('what the page keeps of how long its calls took, and when it sends it', () => {
  it('keeps a call of a route the node takes, and not one it does not list', async () => {
    vi.useFakeTimers()
    const telemetry = createTelemetry()
    const { reports, report } = taken()
    telemetry.record(known)
    telemetry.record({ route: 'POST /api/telemetry', status: '2xx', ms: 3 })
    telemetry.record({ route: 'GET /api/orgs/acme/teams/app/view', status: '2xx', ms: 3 })
    const stop = telemetry.start(report)
    await vi.advanceTimersByTimeAsync(REPORT_EVERY_MS)
    expect(reports).toEqual([[known]])
    stop()
  })

  it('sends once a minute, and not at all when nothing has been kept', async () => {
    vi.useFakeTimers()
    const telemetry = createTelemetry()
    const { reports, report } = taken()
    const stop = telemetry.start(report)
    await vi.advanceTimersByTimeAsync(REPORT_EVERY_MS * 2)
    expect(reports).toEqual([])
    telemetry.record(known)
    await vi.advanceTimersByTimeAsync(REPORT_EVERY_MS - 1)
    expect(reports).toEqual([])
    await vi.advanceTimersByTimeAsync(1)
    expect(reports).toEqual([[known]])
    stop()
  })

  it('sends at once when the page is hidden, and when it is closed, so that the last minute is not lost', async () => {
    vi.useFakeTimers()
    const telemetry = createTelemetry()
    const { reports, report } = taken()
    const stop = telemetry.start(report)
    telemetry.record(known)
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'visible' })
    document.dispatchEvent(new Event('visibilitychange'))
    await vi.advanceTimersByTimeAsync(0)
    expect(reports).toEqual([])
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' })
    document.dispatchEvent(new Event('visibilitychange'))
    await vi.advanceTimersByTimeAsync(0)
    expect(reports).toEqual([[known]])
    telemetry.record({ ...known, ms: 99 })
    window.dispatchEvent(new Event('pagehide'))
    await vi.advanceTimersByTimeAsync(0)
    expect(reports).toEqual([[known], [{ ...known, ms: 99 }]])
    stop()
  })

  it('sends nothing after it is stopped', async () => {
    vi.useFakeTimers()
    const telemetry = createTelemetry()
    const { reports, report } = taken()
    telemetry.record(known)
    telemetry.start(report)()
    await vi.advanceTimersByTimeAsync(REPORT_EVERY_MS * 2)
    window.dispatchEvent(new Event('pagehide'))
    await vi.advanceTimersByTimeAsync(0)
    expect(reports).toEqual([])
  })

  it('forgets what it kept, keeps nothing more and never sends when the viewer is not the owner', async () => {
    vi.useFakeTimers()
    const telemetry = createTelemetry()
    const { reports, report } = taken()
    telemetry.record(known)
    telemetry.forbid()
    telemetry.record(known)
    const stop = telemetry.start(report)
    await vi.advanceTimersByTimeAsync(REPORT_EVERY_MS * 2)
    window.dispatchEvent(new Event('pagehide'))
    await vi.advanceTimersByTimeAsync(0)
    expect(reports).toEqual([])
    stop()
  })
})
