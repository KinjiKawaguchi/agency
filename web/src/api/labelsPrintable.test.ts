import { createApiClient } from './client'

const dirty = 'Al\u202Ece'
const json = (body: unknown) => Promise.resolve(new Response(JSON.stringify(body), { headers: { 'content-type': 'application/json' } }))

describe('the labels the page reads from the node', () => {
  it('are printable in every answer: the viewer, the devices, a team’s view, the decision log', async () => {
    const api = createApiClient((request) => {
      const path = new URL(request.url, 'http://mock.local').pathname
      if (path === '/api/me') return json({ account: { id: 'alice', label: dirty, fingerprint: 'f' }, orgs: [{ id: 'acme', label: dirty, isRoot: true, teams: [] }], funding: [] })
      if (path === '/api/devices') return json({ orgs: [{ org: 'acme', devices: [{ id: 'd-1', label: dirty }] }], requests: [{ label: dirty }] })
      if (path.endsWith('/view')) return json({ view: { label: dirty, people: [{ id: 'bob', label: dirty }], requests: [{ id: 'r-1', title: dirty }] }, mine: [], relayedFor: { 'r-1': dirty } })
      return json({ entries: [{ id: 'e-1', choiceLabel: dirty, title: dirty, relayedFor: { id: 'finance', label: dirty } }] })
    })
    expect((await api.me()).account.label).toBe('Al?ce')
    expect((await api.me()).orgs[0]?.label).toBe('Al?ce')
    const devices = await api.devices()
    expect([devices.orgs[0]?.devices[0]?.label, devices.requests[0]?.label]).toEqual(['Al?ce', 'Al?ce'])
    const snapshot = await api.teamSnapshot('acme', 'app')
    expect(snapshot.view.label).toBe('Al?ce')
    expect(snapshot.view.people[0]?.label).toBe('Al?ce')
    expect(snapshot.relayedFor).toEqual({ 'r-1': 'Al?ce' })
    expect(snapshot.view.requests[0]?.title, 'a title is not a label').toBe(dirty)
    const log = await api.decisions(10)
    expect(log.entries[0]).toMatchObject({ choiceLabel: 'Al?ce', relayedFor: { id: 'finance', label: 'Al?ce' } })
  })

  it('are not touched in what the page says about an error: a refusal’s own words stay', async () => {
    const api = createApiClient(() => Promise.resolve(new Response(JSON.stringify({ error: `no ${dirty}` }), { status: 403, headers: { 'content-type': 'application/json' } })))
    await expect(api.me()).rejects.toThrow(`no ${dirty}`)
  })
})
