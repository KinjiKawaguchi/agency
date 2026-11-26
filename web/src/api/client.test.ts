import { buildEnvelope, newEnvelopeId, signEnvelope } from '../device/envelope'
import { createTestNode, TEST_DEVICE, TEST_DEVICE_ID } from '../test/testDevice'
import { ApiError, createApiClient } from './client'
import { createMockNode } from '../mock/node'

const ANSWER_WITH = { kind: 'answer', target: 'r-20261124-113000-c3d4' } as const

/** A decision signed by the test device and sent with the given notes; answers the log entry the node kept. */
async function sent(notes: { title?: string; choiceLabel?: string }, choice?: string) {
  const node = createTestNode()
  const api = createApiClient((request) => node.handle(request))
  const answer = { org: 'acme', team: 'app', answerWith: ANSWER_WITH, title: 'unused', text: 'Go.', ...(choice === undefined ? {} : { choice }) }
  const envelope = await signEnvelope(TEST_DEVICE, TEST_DEVICE_ID, buildEnvelope({ answer, account: 'alice', id: newEnvelopeId(), at: new Date() }))
  await api.decideSigned('acme', 'app', { envelope, ...notes })
  return (await api.decisions(1)).entries[0]
}

describe('decideSigned', () => {
  it('sends a title of exactly 200 characters as it is', async () => {
    expect((await sent({ title: `  ${'x'.repeat(200)}  ` }))?.title).toBe('x'.repeat(200))
  })

  it('cuts a title of 201 characters so that it and its ellipsis make 200, and the node accepts it', async () => {
    const entry = await sent({ title: 'x'.repeat(201) })
    expect(entry?.title).toBe(`${'x'.repeat(199)}…`)
    expect([...(entry?.title ?? '')]).toHaveLength(200)
  })

  it('counts characters, not UTF-16 units: emoji near the limit are neither split nor over-counted', async () => {
    // 200 emoji are 400 UTF-16 units but exactly 200 characters, so the node takes them whole.
    expect((await sent({ title: '🙂'.repeat(200) }))?.title).toBe('🙂'.repeat(200))
    // 201 are cut to 199 whole emoji and the ellipsis, never leaving half of one.
    const cut = (await sent({ title: '🙂'.repeat(201) }))?.title
    expect(cut).toBe(`${'🙂'.repeat(199)}…`)
    expect([...(cut ?? '')]).toHaveLength(200)
  })

  it('does not leave a space before the ellipsis', async () => {
    expect((await sent({ title: `${'x'.repeat(198)} ${'y'.repeat(10)}` }))?.title).toBe(`${'x'.repeat(198)}…`)
  })

  it('sends the label of the chosen option along with the choice, trimmed and cut to 200 characters', async () => {
    const entry = await sent({ choiceLabel: `  Approve ${'x'.repeat(300)}` }, 'approve')
    expect(entry).toMatchObject({ choice: 'approve' })
    expect([...(entry?.choiceLabel ?? '')]).toHaveLength(200)
    expect(entry?.choiceLabel?.startsWith('Approve x')).toBe(true)
    expect(entry?.choiceLabel?.endsWith('…')).toBe(true)
  })

  it('sends no label without a choice, which the node would refuse', async () => {
    expect(await sent({ choiceLabel: 'Approve' })).not.toHaveProperty('choiceLabel')
  })

  it('sends no label that is empty once trimmed', async () => {
    expect(await sent({ choiceLabel: '   ' }, 'approve')).not.toHaveProperty('choiceLabel')
  })
})

describe('the write token, which changes each time the node starts', () => {
  const PUSH = { token: 'a'.repeat(64), environment: 'sandbox', label: 'Phone' } as const

  /** A client whose calls are counted by method and path, against the given node. */
  function counted(node: ReturnType<typeof createMockNode>) {
    const calls: string[] = []
    const api = createApiClient((request) => {
      calls.push(`${request.method} ${new URL(request.url, 'http://mock.local').pathname}`)
      return node.handle(request)
    })
    return { api, calls }
  }

  it('asks the node for the token once when a write is refused for it, and sends the write again', async () => {
    const node = createMockNode()
    const { api, calls } = counted(node)
    node.restart()
    await api.registerPushToken(PUSH)
    expect(node.pushTokens()).toHaveLength(1)
    expect(calls).toEqual(['POST /api/push/tokens', 'GET /api/session', 'POST /api/push/tokens'])
  })

  it('keeps the new token for the writes that follow, without asking again', async () => {
    const node = createMockNode()
    const { api, calls } = counted(node)
    node.restart()
    await api.registerPushToken(PUSH)
    await api.registerPushToken({ ...PUSH, token: 'b'.repeat(64) })
    expect(calls.filter((call) => call === 'GET /api/session')).toHaveLength(1)
    expect(calls.filter((call) => call === 'POST /api/push/tokens')).toHaveLength(3)
  })

  it('sends a write again only once: a second refusal is the answer', async () => {
    const calls: string[] = []
    const refusing = createApiClient((request) => {
      const path = new URL(request.url, 'http://mock.local').pathname
      calls.push(`${request.method} ${path}`)
      return Promise.resolve(path === '/api/session' ? new Response(JSON.stringify({ writeToken: 'next', now: '2026-11-24T12:00:00Z' }), { status: 200 }) : new Response(JSON.stringify({ error: 'missing or wrong write token' }), { status: 403 }))
    })
    await expect(refusing.registerPushToken(PUSH)).rejects.toMatchObject({ status: 403 })
    expect(calls).toEqual(['POST /api/push/tokens', 'GET /api/session', 'POST /api/push/tokens'])
  })

  it('does not ask again for a refusal that is not about the token, or an answer that is not a 403', async () => {
    for (const [status, message] of [[403, 'this account is not the owner'], [409, 'missing or wrong write token'], [400, 'bad body']] as const) {
      const calls: string[] = []
      const api = createApiClient((request) => {
        calls.push(`${request.method} ${new URL(request.url, 'http://mock.local').pathname}`)
        return Promise.resolve(new Response(JSON.stringify({ error: message }), { status }))
      })
      await expect(api.registerPushToken(PUSH)).rejects.toBeInstanceOf(ApiError)
      expect(calls, `${status} ${message}`).toEqual(['POST /api/push/tokens'])
    }
  })
})
