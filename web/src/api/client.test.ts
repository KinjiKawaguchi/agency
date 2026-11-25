import { buildEnvelope, newEnvelopeId, signEnvelope } from '../device/envelope'
import { createTestNode, TEST_DEVICE, TEST_DEVICE_ID } from '../test/testDevice'
import { createApiClient } from './client'

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
  it('sends the request’s title, trimmed and cut to the 200 characters the node keeps', async () => {
    const entry = await sent({ title: `  ${'あ'.repeat(199)}🙂🙂` })
    expect([...(entry?.title ?? '')]).toHaveLength(200)
    expect(entry?.title).toBe(`${'あ'.repeat(199)}🙂`)
  })

  it('sends the label of the chosen option along with the choice, trimmed and cut to 200 characters', async () => {
    const entry = await sent({ choiceLabel: `  Approve ${'x'.repeat(300)}` }, 'approve')
    expect(entry).toMatchObject({ choice: 'approve' })
    expect(entry?.choiceLabel).toHaveLength(200)
    expect(entry?.choiceLabel?.startsWith('Approve x')).toBe(true)
  })

  it('sends no label without a choice, which the node would refuse', async () => {
    expect(await sent({ choiceLabel: 'Approve' })).not.toHaveProperty('choiceLabel')
  })

  it('sends no label that is empty once trimmed', async () => {
    expect(await sent({ choiceLabel: '   ' }, 'approve')).not.toHaveProperty('choiceLabel')
  })
})
