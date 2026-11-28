// The mock node's `POST /api/telemetry`, as the node takes it (`docs/protocol.md`, "What a client reports"): a well formed report of at most
// 200 samples and 16 KiB, a sample of a route the node lists and a status a series may have is kept, any other sample is dropped and counted,
// and a report that is not well formed is refused whole (400, 413) with nothing kept.
import table from '../../../testdata/route-templates.json' with { type: 'json' }
import type { TelemetryAnswer } from '../api/clientApi'

const MAX_SAMPLES = 200
const MAX_BYTES = 16 * 1024
const CLIENTS: ReadonlySet<string> = new Set(['web', 'ios'])
const STATUSES: ReadonlySet<string> = new Set(['2xx', '3xx', '4xx', '5xx', 'network'])
const ROUTES: ReadonlySet<string> = new Set(table.routes)

/** A sample the node kept, with the client that reported it. */
export interface KeptSample {
  client: string
  route: string
  status: string
  ms: number
}

const json = (body: unknown, status = 200): Response => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })

interface Given {
  route: string
  status: string
  ms: number
}

const isGiven = (value: unknown): value is Given =>
  typeof value === 'object' && value !== null && 'route' in value && typeof value.route === 'string' && 'status' in value && typeof value.status === 'string' && 'ms' in value && typeof value.ms === 'number' && Number.isInteger(value.ms) && value.ms >= 0

export function createMockTelemetry() {
  let kept: readonly KeptSample[] = []
  return {
    /** What the node has kept of the reports it took, oldest first. */
    kept: (): readonly KeptSample[] => kept,
    /** Answers `POST /api/telemetry`, or nothing when the request is another route. `write` is the node's check of the write token and the JSON body. */
    handle(request: Request, url: URL, write: () => Promise<Response | { body: unknown }>): Promise<Response> | undefined {
      if (request.method !== 'POST' || url.pathname !== '/api/telemetry') return undefined
      return write().then((answer) => {
        if (answer instanceof Response) return answer
        const report = answer.body
        if (JSON.stringify(report).length > MAX_BYTES) return json({ error: 'The report is larger than 16 KiB.' }, 413)
        if (typeof report !== 'object' || report === null || !('client' in report) || typeof report.client !== 'string' || !CLIENTS.has(report.client)) return json({ error: 'The report is not well formed.' }, 400)
        const samples: unknown = 'samples' in report ? report.samples : undefined
        if (!Array.isArray(samples) || samples.length > MAX_SAMPLES || !samples.every(isGiven)) return json({ error: 'The report is not well formed.' }, 400)
        const client = report.client
        const known = samples.filter((sample) => ROUTES.has(sample.route) && STATUSES.has(sample.status))
        kept = [...kept, ...known.map((sample) => ({ client, route: sample.route, status: sample.status, ms: sample.ms }))]
        return json({ accepted: known.length, dropped: samples.length - known.length } satisfies TelemetryAnswer)
      })
    },
  }
}
