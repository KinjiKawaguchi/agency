import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

/** The chart box of a kind and name, such as ('Department', 'Design'). Its text contains both, one after the other. */
const box = (kind: string, name: string): Promise<HTMLElement> =>
  waitFor(() => {
    const found = screen.getAllByRole('link').find((link) => link.textContent?.includes(`${kind}${name}`))
    if (found === undefined) throw new Error(`No ${kind} ${name}`)
    return found
  })

describe('org chart', () => {
  it('draws the organization with its departments, teams, and people', async () => {
    renderApp({ path: '/org' })
    const chart = within(await screen.findByRole('group', { name: 'Organization chart' }))
    expect(chart.getByText('Acme')).toBeInTheDocument()
    expect(await box('Department', 'Engineering')).toHaveTextContent('Led by Alice')
    expect(await box('Department', 'Platform')).toHaveTextContent('Led by Bob')
    expect(await box('Department', 'Design')).toHaveTextContent('Led by Carol')
    expect(chart.getByText('Carol')).toBeInTheDocument()
    expect(chart.getByText('Answers only')).toBeInTheDocument()
  })

  it('marks the viewer', async () => {
    renderApp({ path: '/org' })
    const you = (await screen.findByText('You')).closest('.oc-node')
    expect(you).toHaveTextContent('Alice')
  })

  it('shows the agents under their team with the one waiting for the viewer', async () => {
    renderApp({ path: '/org' })
    const team = await box('Team', 'App')
    expect(team).toHaveTextContent('4 agents, 2 working')
    expect(team).toHaveTextContent('1 waiting for you')
    expect(await box('Backend', 'Agent')).toHaveTextContent('Waiting for your answer')
    expect(await box('Lead', 'Lead (agent)')).toHaveTextContent('Reviewing app#12')
  })

  it('tells a department how many are waiting below it', async () => {
    renderApp({ path: '/org' })
    expect(await box('Department', 'Engineering')).toHaveTextContent('1 waiting for you')
    expect(await box('Department', 'Design')).not.toHaveTextContent('waiting for you')
  })

  it('focuses on a department and comes back to the whole organization', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/org' })
    await user.click(await box('Department', 'Engineering'))
    expect(await screen.findByRole('link', { name: 'Back to the whole organization' })).toBeInTheDocument()
    expect(await box('Department', 'Platform')).toBeInTheDocument()
    expect(screen.queryByText('Design')).not.toBeInTheDocument()
    await user.click(screen.getByRole('link', { name: 'Back to the whole organization' }))
    expect(await box('Department', 'Design')).toBeInTheDocument()
  })

  it('shows the whole organization for a department that does not exist', async () => {
    renderApp({ path: '/org?unit=nowhere' })
    expect(await box('Department', 'Design')).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: 'Back to the whole organization' })).not.toBeInTheDocument()
  })
})
