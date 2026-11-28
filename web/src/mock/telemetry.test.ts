import { createMockNode } from './node'

/** A write to the mock node as the page makes it: with the write token its `GET /api/session` gives, and JSON. */
async function report(node: ReturnType<typeof createMockNode>, body: unknown, headers: Record<string, string> = {}): Promise<Response> {
  const session = (await (await node.handle(new Request('http://mock.local/api/session'))).json()) as { writeToken: string }
  return node.handle(new Request('http://mock.local/api/telemetry', { method: 'POST', headers: { 'content-type': 'application/json', 'x-agency-token': session.writeToken, ...headers }, body: JSON.stringify(body) }))
}

const sample = { route: 'GET /api/me', status: '2xx', ms: 12 }

describe('the mock node’s report route, as the node’s is', () => {
  it('keeps a sample of a listed route and status under its client, and drops and counts the rest', async () => {
    const node = createMockNode()
    const answer = await report(node, { client: 'web', samples: [sample, { ...sample, route: 'GET /api/orgs/acme/teams/app/view' }, { ...sample, status: '9xx' }, { ...sample, ms: 0, status: 'network' }] })
    expect(answer.status).toBe(200)
    expect(await answer.json()).toEqual({ accepted: 2, dropped: 2 })
    expect(node.telemetry()).toEqual([
      { client: 'web', ...sample },
      { client: 'web', route: 'GET /api/me', status: 'network', ms: 0 },
    ])
  })

  it('takes an empty report and 200 samples', async () => {
    const node = createMockNode()
    expect((await report(node, { client: 'ios', samples: [] })).status).toBe(200)
    expect((await report(node, { client: 'ios', samples: Array.from({ length: 200 }, () => sample) })).status).toBe(200)
    expect(node.telemetry()).toHaveLength(200)
  })

  it.each([
    ['an unknown client', { client: 'desktop', samples: [sample] }],
    ['no client', { samples: [sample] }],
    ['no samples', { client: 'web' }],
    ['201 samples', { client: 'web', samples: Array.from({ length: 201 }, () => sample) }],
    ['a negative time', { client: 'web', samples: [{ ...sample, ms: -1 }] }],
    ['a time with a fraction', { client: 'web', samples: [{ ...sample, ms: 1.5 }] }],
    ['a time that is text', { client: 'web', samples: [{ ...sample, ms: '12' }] }],
    ['a route that is not text', { client: 'web', samples: [{ ...sample, route: 7 }] }],
    ['a body that is not an object', 'report'],
  ])('refuses a report with %s whole (400) and keeps nothing of it', async (_name, body) => {
    const node = createMockNode()
    expect((await report(node, body)).status).toBe(400)
    expect(node.telemetry()).toEqual([])
  })

  it('refuses a report of more than 16 KiB (413) and keeps nothing of it', async () => {
    const node = createMockNode()
    expect((await report(node, { client: 'web', samples: [{ ...sample, route: 'GET /api/me'.padEnd(17_000, 'x') }] })).status).toBe(413)
    expect(node.telemetry()).toEqual([])
  })

  it('is a write like the others: it needs the write token and says JSON', async () => {
    const node = createMockNode()
    expect((await report(node, { client: 'web', samples: [sample] }, { 'x-agency-token': 'wrong' })).status).toBe(403)
    expect((await report(node, { client: 'web', samples: [sample] }, { 'content-type': 'text/plain' })).status).toBe(415)
    expect(node.telemetry()).toEqual([])
  })

  it('is the owner’s: a hosted member is refused (403)', async () => {
    const node = createMockNode({ guest: { member: 'dave', label: 'Dave' } })
    expect((await report(node, { client: 'web', samples: [sample] })).status).toBe(403)
    expect(node.telemetry()).toEqual([])
  })
})
