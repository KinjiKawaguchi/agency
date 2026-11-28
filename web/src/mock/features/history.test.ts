import { createMockNode } from '../node'

const get = (node: ReturnType<typeof createMockNode>, path: string) => node.handle(new Request(`http://mock.local${path}`))
const versions = async (node: ReturnType<typeof createMockNode>, query = ''): Promise<{ entries: Array<{ version: number }>; more?: boolean }> => (await (await get(node, `/api/orgs/acme/history${query}`)).json()) as { entries: Array<{ version: number }>; more?: boolean }

describe('the mock node’s history route, as the node’s is', () => {
  it('lists the capability and answers the whole history oldest first, with no `more`, when it fits in a page', async () => {
    const node = createMockNode({ history: 3 })
    expect(((await (await get(node, '/api/about')).json()) as { capabilities: string[] }).capabilities).toContain('history')
    const answer = await versions(node)
    expect(answer.entries.map((entry) => entry.version)).toEqual([1, 2, 3])
    expect(answer.more).toBeUndefined()
  })

  it('cuts the answer at `limit` and says `more`, and `after` goes on from the last version read, so that pages join into the whole history', async () => {
    const node = createMockNode({ history: 5 })
    const first = await versions(node, '?limit=2')
    expect(first).toMatchObject({ entries: [{ version: 1 }, { version: 2 }], more: true })
    const second = await versions(node, '?limit=2&after=2')
    expect(second).toMatchObject({ entries: [{ version: 3 }, { version: 4 }], more: true })
    const last = await versions(node, '?limit=2&after=4')
    expect(last.entries.map((entry) => entry.version)).toEqual([5])
    expect(last.more).toBeUndefined()
  })

  it.each(['limit=0', 'limit=1001', 'limit=x', 'after=x', 'after=-1', 'limit=1.5'])('refuses %s as the node does (400)', async (query) => {
    expect((await get(createMockNode({ history: 3 }), `/api/orgs/acme/history?${query}`)).status).toBe(400)
  })

  it('answers 404 as a node without the capability does, and for an org it does not have', async () => {
    expect((await get(createMockNode(), '/api/orgs/acme/history')).status).toBe(404)
    expect(((await (await get(createMockNode(), '/api/about')).json()) as { capabilities: string[] }).capabilities).not.toContain('history')
    expect((await get(createMockNode({ history: 3 }), '/api/orgs/other/history')).status).toBe(404)
  })

  it('is the owner’s: a hosted member is refused', async () => {
    expect((await get(createMockNode({ history: 3, guest: { member: 'dave', label: 'Dave' } }), '/api/orgs/acme/history')).status).toBe(403)
  })
})
