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
