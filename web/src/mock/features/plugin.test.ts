import { nodeOptions } from '../../../vite/mock-node-plugin'

/** What a running mock node (`vite preview`) is made with, from the query of the reset that e2e sends: the features are asked, in their own terms. */
describe('the options of the running mock node, read from the query of a reset', () => {
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
