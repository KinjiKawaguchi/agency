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
})
