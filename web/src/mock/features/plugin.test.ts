import { afterEach, beforeEach, vi } from 'vitest'
import { nodeOptions } from '../../../vite/mock-node-plugin'

/** The settings of the shell that the running node reads when the query does not give them: a test of what is read from the query says what the shell holds. */
const SHELL = ['AGENCY_MOCK_ACTIVITY', 'AGENCY_MOCK_EVALUATION', 'AGENCY_MOCK_METRICS', 'AGENCY_MOCK_OUTCOMES', 'AGENCY_MOCK_PARTICIPANTS', 'AGENCY_MOCK_USAGE', 'AGENCY_MOCK_WEBPUSH', 'AGENCY_MOCK_WEBPUSH_FAILING'] as const
beforeEach(() => {
  for (const name of SHELL) vi.stubEnv(name, '')
})
afterEach(() => {
  vi.unstubAllEnvs()
})

/** What a running mock node (`vite preview`) is made with, from the query of the reset that e2e sends: the features are asked, in their own terms. */
describe('the options of the running mock node, read from the query of a reset', () => {
  it('has nothing from the shell when the shell holds no setting, and the setting of the shell when the query gives none', () => {
    vi.stubEnv('AGENCY_MOCK_EVALUATION', '1')
    expect(nodeOptions(new URLSearchParams(''))).toHaveProperty('budget', true)
    expect(nodeOptions(new URLSearchParams('evaluation=0'))).not.toHaveProperty('budget')
  })

  it('has no feature of the Client API in a node that was asked for none', () => {
    expect(nodeOptions(new URLSearchParams(''))).not.toHaveProperty('budget')
    expect(nodeOptions(new URLSearchParams(''))).not.toHaveProperty('history')
    expect(nodeOptions(new URLSearchParams(''))).not.toHaveProperty('members')
  })

  it('has the budget with the evaluation, since the screen that shows it is the evaluation’s, and not without it', () => {
    expect(nodeOptions(new URLSearchParams('evaluation=1'))).toHaveProperty('budget', true)
    expect(nodeOptions(new URLSearchParams('evaluation=0'))).not.toHaveProperty('budget')
    expect(nodeOptions(new URLSearchParams('budgetState=over'))).not.toHaveProperty('budget')
  })

  it('fixes where the app team stands for the budget when asked, and has what the other features were asked for', () => {
    expect(nodeOptions(new URLSearchParams('evaluation=1&budgetState=over'))).toMatchObject({ budget: { app: { team: { state: 'over' } } } })
    expect(nodeOptions(new URLSearchParams('members=1&history=4'))).toMatchObject({ members: true, history: 4 })
  })
})
