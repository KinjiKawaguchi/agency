import { buildEnvelope, newEnvelopeId, signEnvelope } from '../device/envelope'
import { createTestNode, TEST_DEVICE, TEST_DEVICE_ID } from '../test/testDevice'
import { MOCK_TOKEN, createMockNode } from './node'

const ANSWER = { org: 'acme', team: 'app', answerWith: { kind: 'answer', target: 'r-20261124-113000-c3d4' }, title: 'A request', choice: 'approve', text: 'Go.' } as const

async function body(): Promise<string> {
  const envelope = buildEnvelope({ answer: ANSWER, account: 'alice', id: newEnvelopeId(), at: new Date() })
  return JSON.stringify({ envelope: await signEnvelope(TEST_DEVICE, TEST_DEVICE_ID, envelope) })
}

const post = async (headers: Record<string, string>) =>
  new Request('http://mock.local/api/orgs/acme/teams/app/envelopes', { method: 'POST', headers, body: await body() })

describe('mock node writes', () => {
  it('accepts a signed decision with the page token and a JSON body', async () => {
    const node = createTestNode()
    const reply = await node.handle(await post({ 'content-type': 'application/json', 'x-agency-token': MOCK_TOKEN }))
    expect(reply.status).toBe(200)
    expect(await reply.json()).toEqual({ outcome: 'applied' })
  })

  it('refuses a decision without the token, and does not apply it', async () => {
    const node = createTestNode()
    const reply = await node.handle(await post({ 'content-type': 'application/json' }))
    expect(reply.status).toBe(403)
    expect(node.decisions()).toEqual([])
  })

  it('refuses a decision with the wrong token', async () => {
    const node = createTestNode()
    expect((await node.handle(await post({ 'content-type': 'application/json', 'x-agency-token': 'guess' }))).status).toBe(403)
    expect(node.decisions()).toEqual([])
  })

  it('refuses a body that is not declared as JSON', async () => {
    const node = createTestNode()
    expect((await node.handle(await post({ 'content-type': 'text/plain', 'x-agency-token': MOCK_TOKEN }))).status).toBe(415)
    expect(node.decisions()).toEqual([])
  })

  it('no longer takes a decision it would have to sign itself', async () => {
    const node = createMockNode()
    const reply = await node.handle(new Request('http://mock.local/api/orgs/acme/teams/app/decisions', { method: 'POST', headers: { 'content-type': 'application/json', 'x-agency-token': MOCK_TOKEN }, body: '{}' }))
    expect(reply.status).toBe(404)
  })
})

describe('mock node reads', () => {
  it('serves a team as a snapshot with the viewer and where it came from', async () => {
    const reply = await createMockNode().handle(new Request('http://mock.local/api/orgs/acme/teams/site/view'))
    const body = (await reply.json()) as { view: { team: string }; viewer: string; source: { kind: string } }
    expect(body).toMatchObject({ view: { team: 'site' }, viewer: 'owner', source: { kind: 'replica' } })
  })

  it('serves the org definition at the org itself', async () => {
    const reply = await createMockNode().handle(new Request('http://mock.local/api/orgs/acme'))
    expect(await reply.json()).toMatchObject({ org: 'acme', label: 'Acme' })
  })

  it('serves funding with the viewer’s own account details', async () => {
    const reply = await createMockNode().handle(new Request('http://mock.local/api/me'))
    const me = (await reply.json()) as { account: { fingerprint: string }; funding: Array<{ windows: unknown[] }> }
    expect(me.account.fingerprint).not.toBe('')
    expect(me.funding[0]?.windows).toHaveLength(2)
  })
})
