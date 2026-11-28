import { createMockNode } from '../node'

const get = (node: ReturnType<typeof createMockNode>, path: string) => node.handle(new Request(`http://mock.local${path}`))

describe('the mock node’s members route', () => {
  it('lists the capability and answers the node’s own sample for the org it describes', async () => {
    const node = createMockNode({ members: true })
    expect(((await (await get(node, '/api/about')).json()) as { capabilities: string[] }).capabilities).toContain('members')
    const answer = (await (await get(node, '/api/orgs/acme/members')).json()) as { org: string; members: Array<{ id: string }> }
    expect(answer.org).toBe('acme')
    expect(answer.members.map((member) => member.id)).toEqual(['alice', 'bob', 'carol'])
  })

  it('answers 404 as a node without the capability does, and for an org it has no sample for', async () => {
    expect((await get(createMockNode(), '/api/orgs/acme/members')).status).toBe(404)
    expect(((await (await get(createMockNode(), '/api/about')).json()) as { capabilities: string[] }).capabilities).not.toContain('members')
    expect((await get(createMockNode({ members: true }), '/api/orgs/other/members')).status).toBe(404)
  })

  it('is the owner’s: a hosted member is refused', async () => {
    expect((await get(createMockNode({ members: true, guest: { member: 'dave', label: 'Dave' } }), '/api/orgs/acme/members')).status).toBe(403)
  })
})
