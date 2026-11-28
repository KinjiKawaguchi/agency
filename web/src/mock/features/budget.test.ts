import { createMockNode } from '../node'

const get = (node: ReturnType<typeof createMockNode>, path: string) => node.handle(new Request(`http://mock.local${path}`))
const now = new Date('2026-11-25T12:00:00Z')

describe('the mock node’s weekly budget route', () => {
  it('answers a node with the capability, which lists it in its about, for the week from Monday', async () => {
    const node = createMockNode({ budget: true, now })
    expect(((await (await get(node, '/api/about')).json()) as { capabilities: string[] }).capabilities).toContain('budget')
    const answer = (await (await get(node, '/api/orgs/acme/teams/app/budget')).json()) as { weekStart: string; team: { state: string; capUsd?: number } }
    expect(answer).toMatchObject({ weekStart: '2026-11-23', team: { capUsd: 100, state: 'warning' } })
  })

  it('is a Sunday’s Monday six days back, and a Monday’s own day', async () => {
    const sunday = createMockNode({ budget: true, now: new Date('2026-11-29T23:00:00Z') })
    const monday = createMockNode({ budget: true, now: new Date('2026-11-23T00:30:00Z') })
    expect(((await (await get(sunday, '/api/orgs/acme/teams/app/budget')).json()) as { weekStart: string }).weekStart).toBe('2026-11-23')
    expect(((await (await get(monday, '/api/orgs/acme/teams/app/budget')).json()) as { weekStart: string }).weekStart).toBe('2026-11-23')
  })

  it('leaves out the cap of a team with no plan', async () => {
    const node = createMockNode({ budget: true, now })
    const answer = (await (await get(node, '/api/orgs/acme/teams/site/budget')).json()) as { team: { capUsd?: number; state: string } }
    expect(answer.team).toEqual({ spentUsd: 147.8, state: 'ok' })
  })

  it('answers 404 as a node without the capability does, and for a team it has no answer for', async () => {
    expect((await get(createMockNode({ now }), '/api/orgs/acme/teams/app/budget')).status).toBe(404)
    expect((await get(createMockNode({ budget: {}, now }), '/api/orgs/acme/teams/app/budget')).status).toBe(404)
  })

  it('is the owner’s: a hosted member is refused', async () => {
    const node = createMockNode({ budget: true, now, guest: { member: 'dave', label: 'Dave' } })
    expect((await get(node, '/api/orgs/acme/teams/app/budget')).status).toBe(403)
  })
})
