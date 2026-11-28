import about from '../../../../testdata/about.sample.json' with { type: 'json' }
import { createMockNode } from '../node'
import { answerOf, capabilitiesOf, isOwnerRoute, optionsFromQuery, queryOfFeatures } from './index'

/**
 * What each feature of the mock node is, asked the same way of all of them: this holds the contract of `MockFeature`, so that a new feature that
 * has the module and the line in `index.ts` is held to it as the others are. Each row is a feature, with how to switch it on, the one route it
 * answers, what a test asks of the node to switch it on, and what the running node reads of that.
 */
const FEATURES = [
  { capability: 'budget', options: { budget: true as const }, path: '/api/orgs/acme/teams/app/budget', asked: { budgetState: 'over' }, query: [['budgetState', 'over']], readFrom: 'evaluation=1&budgetState=over', read: { budget: { app: { team: { state: 'over' } } } } },
  { capability: 'history', options: { history: 3 }, path: '/api/orgs/acme/history', asked: { history: 3 }, query: [['history', '3']], readFrom: 'history=3', read: { history: 3 } },
  { capability: 'members', options: { members: true as const }, path: '/api/orgs/acme/members', asked: { members: true }, query: [['members', '1']], readFrom: 'members=1', read: { members: true } },
]

const ask = (path: string) => new Request(`http://mock.local${path}`)
const now = new Date('2026-11-25T12:00:00Z')

describe.each(FEATURES)('the $capability feature of the mock node', ({ capability, options, path, asked, query, readFrom, read }) => {
  it('says its capability when it is on, and only then', () => {
    expect(capabilitiesOf(options)).toContain(capability)
    expect(capabilitiesOf({})).not.toContain(capability)
  })

  it('answers its route when it is on, and does not when it is off: the node then answers 404 as one without it does', async () => {
    expect(answerOf(ask(path), new URL(ask(path).url), options, { now })?.status).toBe(200)
    expect(answerOf(ask(path), new URL(ask(path).url), {}, { now })).toBeUndefined()
    expect((await createMockNode().handle(ask(path))).status).toBe(404)
  })

  it('leaves what is not its route to the others', () => {
    expect(answerOf(ask('/api/nothing-of-it'), new URL('http://mock.local/api/nothing-of-it'), options, { now })).toBeUndefined()
  })

  it('has its route among the owner’s, which a hosted member is refused', async () => {
    expect(isOwnerRoute(path)).toBe(true)
    expect((await createMockNode({ ...options, guest: { member: 'dave', label: 'Dave' } }).handle(ask(path))).status).toBe(403)
  })

  it('is asked for by a test and read by the running node in the same terms', () => {
    expect(queryOfFeatures(asked)).toEqual(expect.arrayContaining(query))
    expect(optionsFromQuery(new URLSearchParams(readFrom))).toMatchObject(read)
    expect(optionsFromQuery(new URLSearchParams(''))).not.toHaveProperty(Object.keys(options)[0] ?? '')
  })

  it('names a capability that the node itself lists (the sample of its `about`), so that the mock says nothing the node does not', () => {
    expect(about.capabilities).toContain(capability)
  })
})

describe('the features together', () => {
  it('say each capability once, and the routes of the features are not each other’s', () => {
    const said = capabilitiesOf({ budget: true, history: 1, members: true })
    expect(new Set(said).size).toBe(said.length)
    for (const one of FEATURES) for (const other of FEATURES) if (one !== other) expect(answerOf(ask(one.path), new URL(ask(one.path).url), other.options, { now })).toBeUndefined()
  })
})
