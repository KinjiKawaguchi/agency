import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { createApiClient } from './client'
import { at } from './routes'

describe('the target of a route template', () => {
  it('builds the path from the template, each value as one path segment, and keeps the template for what is reported', () => {
    expect(at('GET /api/orgs/{org}/teams/{team}/view', { org: 'acme', team: 'app' })).toEqual({ route: 'GET /api/orgs/{org}/teams/{team}/view', path: '/api/orgs/acme/teams/app/view' })
    expect(at('GET /api/me')).toEqual({ route: 'GET /api/me', path: '/api/me' })
    expect(at('DELETE /api/push/subscriptions/{id}', { id: 'a/b c?' }).path).toBe('/api/push/subscriptions/a%2Fb%20c%3F')
  })

  it('writes the query, each value encoded, and leaves out a value that is not there, never writing it as "undefined"', () => {
    expect(at('GET /api/metrics', {}, { days: 7, hourBuckets: 1 }).path).toBe('/api/metrics?days=7&hourBuckets=1')
    expect(at('GET /api/metrics', {}, { days: 1, hourBuckets: undefined }).path).toBe('/api/metrics?days=1')
    expect(at('GET /api/orgs/{org}/teams/{team}/activity', { org: 'acme', team: 'app' }, { since: '2026-11-24T10:00:00Z' }).path).toBe('/api/orgs/acme/teams/app/activity?since=2026-11-24T10%3A00%3A00Z')
    expect(at('GET /api/metrics', {}, {}).path).toBe('/api/metrics')
  })

  it('never has the values in the template that is reported', () => {
    expect(at('GET /api/orgs/{org}', { org: 'acme' }).route).not.toContain('acme')
  })

  it('refuses a part with no value, and a value with no part: a mistake of the code', () => {
    expect(() => at('GET /api/orgs/{org}/teams/{team}/view', { org: 'acme' })).toThrow('takes {org}, {team}')
    expect(() => at('GET /api/orgs/{org}', { org: 'acme', team: 'app' })).toThrow('takes {org}')
    expect(() => at('GET /api/me', { org: 'acme' })).toThrow('takes {}')
    expect(() => at('GET /api/orgs/{org}', { team: 'acme' })).toThrow('takes {org}')
  })
})

/** What the client asks for, as the node sees it: the method and the path with its query. */
async function asked(call: (api: ReturnType<typeof createApiClient>) => Promise<unknown>): Promise<string> {
  let seen = ''
  const api = createApiClient((request) => {
    const url = new URL(request.url, 'http://mock.local')
    seen = `${request.method} ${url.pathname}${url.search}`
    return Promise.resolve(new Response(JSON.stringify({}), { status: 200, headers: { 'content-type': 'application/json' } }))
  })
  await call(api).catch(() => undefined)
  return seen
}

describe('what each call of the client asks the node for', () => {
  // The paths the client asked for before its calls named their templates: a call must still ask for exactly this.
  const cases: ReadonlyArray<[string, (api: ReturnType<typeof createApiClient>) => Promise<unknown>, string]> = [
    ['me', (api) => api.me(), 'GET /api/me'],
    ['teamSnapshot', (api) => api.teamSnapshot('acme', 'app'), 'GET /api/orgs/acme/teams/app/view'],
    ['orgDefinition', (api) => api.orgDefinition('acme'), 'GET /api/orgs/acme'],
    ['usage', (api) => api.usage('acme', 'app', 7), 'GET /api/orgs/acme/teams/app/usage?days=7'],
    ['evaluation', (api) => api.evaluation('acme', 14), 'GET /api/orgs/acme/evaluation?days=14'],
    ['metrics', (api) => api.metrics(7, true), 'GET /api/metrics?days=7&hourBuckets=1'],
    ['metrics without buckets', (api) => api.metrics(30), 'GET /api/metrics?days=30'],
    ['activity', (api) => api.activity('acme', 'app', '2026-11-24T10:00:00Z'), 'GET /api/orgs/acme/teams/app/activity?since=2026-11-24T10%3A00%3A00Z'],
    ['outcomes', (api) => api.outcomes('acme', 30), 'GET /api/orgs/acme/outcomes?days=30'],
    ['readings', (api) => api.readings('acme'), 'GET /api/orgs/acme/outcomes/readings'],
    ['numberHistory', (api) => api.numberHistory('acme', 'revenue', 20), 'GET /api/orgs/acme/outcomes/observations?kpi=revenue&limit=20'],
    ['enterNumber', (api) => api.enterNumber('acme', { kpi: 'revenue', date: '2026-11-24', value: 1 }), 'POST /api/orgs/acme/outcomes/observations'],
    ['effort', (api) => api.effort('acme', 14), 'GET /api/orgs/acme/effort?days=14'],
    ['decisions', (api) => api.decisions(10), 'GET /api/decisions?limit=10'],
    ['registerPushToken', (api) => api.registerPushToken({ token: 'ab', environment: 'sandbox', label: 'x' }), 'POST /api/push/tokens'],
    ['vapidKey', (api) => api.vapidKey(), 'GET /api/push/vapid'],
    ['pushSubscriptions', (api) => api.pushSubscriptions(), 'GET /api/push/subscriptions'],
    ['registerPushSubscription', (api) => api.registerPushSubscription({ endpoint: 'https://p.example/x', p256dh: 'a', auth: 'b', label: 'x' }), 'POST /api/push/subscriptions'],
    ['removePushSubscription', (api) => api.removePushSubscription('a b'), 'DELETE /api/push/subscriptions/a%20b'],
    ['devices', (api) => api.devices(), 'GET /api/devices'],
    ['requestDevice', (api) => api.requestDevice({ label: 'x', publicKey: 'k' } as never), 'POST /api/devices/requests'],
    ['claimState', (api) => api.claimState(), 'GET /api/claim'],
    ['about', (api) => api.about(), 'GET /api/about'],
    ['removePushToken', (api) => api.removePushToken('0123456789abcdef'), 'DELETE /api/push/tokens/0123456789abcdef'],
  ]
  it.each(cases)('%s', async (_name, call, expected) => {
    expect(await asked(call)).toBe(expected)
  })
})

describe('the calls of the client, read from its source', () => {
  const source = readFileSync(join(process.cwd(), 'src', 'api', 'client.ts'), 'utf8')
  const lines = source.split('\n').filter((line) => !/^\s*(\/\/|\*|\/\*)/.test(line))

  it('are all made through `at(`: no path is written out beside a template', () => {
    const written = lines.filter((line) => /['`]\/api\//.test(line) && !/\bat\('/.test(line) && !line.includes('shown-ok(route)'))
    // The one request that is not a call of the API: the probe that asks whether the sign-in is gone, which is not measured.
    expect(written.map((line) => line.trim())).toEqual([expect.stringContaining("url('/api/session')")])
  })

  it('name each template once in a form the node can know: a method, `/api/`, lower-case parts in braces, no value in it', () => {
    const templates = [...lines.join('\n').matchAll(/\bat\('([A-Z]+ \/api\/[^']*)'/g)].map((found) => found[1] ?? '')
    expect(templates.length).toBeGreaterThan(20)
    for (const template of templates) expect(template, template).toMatch(/^(GET|POST|DELETE) \/api\/[a-z]+(\/([a-z-]+|\{[a-z]+\}))*$/)
  })
})
