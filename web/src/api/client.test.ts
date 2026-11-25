import { createMockNode } from '../mock/node'
import { createApiClient } from './client'

const ANSWER_WITH = { kind: 'answer', target: 'r-20261124-113000-c3d4' } as const

describe('decide', () => {
  it('sends the request’s title, cut to the 200 characters the node keeps', async () => {
    const node = createMockNode()
    const api = createApiClient((request) => node.handle(request))
    const title = `${'あ'.repeat(199)}🙂🙂`
    await api.decide({ org: 'acme', team: 'app', answerWith: ANSWER_WITH, title, text: 'Go.' })
    const [entry] = (await api.decisions(1)).entries
    expect([...(entry?.title ?? '')]).toHaveLength(200)
    expect(entry?.title).toBe(`${'あ'.repeat(199)}🙂`)
  })

  const sent = async (answer: Partial<Parameters<ReturnType<typeof createApiClient>['decide']>[0]>) => {
    const node = createMockNode()
    const api = createApiClient((request) => node.handle(request))
    await api.decide({ org: 'acme', team: 'app', answerWith: ANSWER_WITH, title: 'A request', text: 'Go.', ...answer })
    return (await api.decisions(1)).entries[0]
  }

  it('sends the label of the chosen option along with the choice, trimmed and cut to 200 characters', async () => {
    const entry = await sent({ choice: 'approve', choiceLabel: `  Approve ${'x'.repeat(300)}` })
    expect(entry).toMatchObject({ choice: 'approve' })
    expect(entry?.choiceLabel).toHaveLength(200)
    expect(entry?.choiceLabel?.startsWith('Approve x')).toBe(true)
  })

  it('sends no label without a choice, which the node would refuse', async () => {
    expect(await sent({ choiceLabel: 'Approve' })).not.toHaveProperty('choiceLabel')
  })

  it('sends no label that is empty once trimmed', async () => {
    expect(await sent({ choice: 'approve', choiceLabel: '   ' })).not.toHaveProperty('choiceLabel')
  })
})
