import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'

describe('people and work', () => {
  it('shows a section for each team, with the viewer left out of the people', async () => {
    renderApp({ path: '/people' })
    expect(await screen.findByRole('heading', { level: 2, name: 'App team' })).toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 2, name: 'Site team' })).toBeInTheDocument()
    expect(screen.getByText('2 of 4 participants are working. 1 is waiting for your answer.')).toBeInTheDocument()
    expect(screen.queryByText('Three requests are waiting for an answer')).not.toBeInTheDocument()
  })

  it('says what a person is doing, and for someone waiting on the viewer, what for', async () => {
    renderApp({ path: '/people' })
    const card = (await screen.findByText('Permission prompt: run the database migration')).closest('article')
    expect(card).not.toBeNull()
    const backend = within(card as HTMLElement)
    expect(backend.getByText('Backend')).toBeInTheDocument()
    expect(backend.getByText('Your turn')).toBeInTheDocument()
    expect(backend.getByText('Waiting for your answer: “Which database should the app use in production?”')).toBeInTheDocument()
    expect(backend.getByRole('link', { name: 'Open decisions' })).toHaveAttribute('href', '/desk')
    expect(backend.getByText('app#12')).toBeInTheDocument()
  })

  it('lists work in progress with its forecast and blocked work with its reason', async () => {
    renderApp({ path: '/people' })
    const doing = (await screen.findByText('Store sessions in the database')).closest('.work-row')
    expect(doing).toHaveTextContent('Expected around')
    const blocked = screen.getByText('Remove the old export format').closest('.work-row')
    expect(blocked).toHaveTextContent('Needs a decision since Nov 23')
    expect(screen.getByText('Document the session store').closest('.work-row')).toHaveTextContent('Waiting on earlier work')
  })

  it('draws an epic with its progress, forecast, and blocked children', async () => {
    renderApp({ path: '/people' })
    const epic = (await screen.findByText('Persistent sessions')).closest('.goal')
    const scope = within(epic as HTMLElement)
    expect(scope.getByText('1 / 4')).toBeInTheDocument()
    expect(scope.getByRole('img', { name: '1 of 4 done, 1 in progress, 2 blocked' })).toBeInTheDocument()
    expect(scope.getByText(/Expected to finish around/)).toBeInTheDocument()
    expect(scope.getByText('Blocked: Needs a decision 1, Waiting on earlier work 1')).toBeInTheDocument()
  })

  it('falls back to the forecast basis when an epic has no forecast', async () => {
    renderApp({ path: '/people' })
    const epic = (await screen.findByText('Mobile layout')).closest('.goal')
    expect(epic).toHaveTextContent('No recent throughput to forecast from')
  })

  it('opens the decisions from a person who is waiting', async () => {
    const user = userEvent.setup()
    renderApp({ path: '/people' })
    await user.click(await screen.findByRole('link', { name: 'Open decisions' }))
    expect(await screen.findByRole('heading', { level: 1 })).toHaveTextContent('waiting for your answer')
  })

  describe('a team with a lot of work', () => {
    const builder = async () => within((await screen.findByText('Building the checkout flow')).closest('article') as HTMLElement)

    it('shows the first refs a person holds and folds the rest behind a count', async () => {
      const user = userEvent.setup()
      renderApp({ path: '/people' })
      const card = await builder()
      expect(card.getAllByText(/^site-checkout#/)).toHaveLength(6)
      await user.click(card.getByRole('button', { name: '+8 more' }))
      expect(card.getAllByText(/^site-checkout#/)).toHaveLength(14)
      expect(card.queryByRole('button', { name: /more$/ })).not.toBeInTheDocument()
    })

    it('keeps a person’s few refs as they were, with nothing to unfold', async () => {
      renderApp({ path: '/people' })
      const card = within((await screen.findByText('Permission prompt: run the database migration')).closest('article') as HTMLElement)
      expect(card.getByText('app#12')).toBeInTheDocument()
      expect(card.queryByRole('button', { name: /more$/ })).not.toBeInTheDocument()
    })

    it('shows the first rows of work in progress and folds the rest behind a count', async () => {
      const user = userEvent.setup()
      renderApp({ path: '/people' })
      await screen.findByText('Building the checkout flow')
      const rows = () => screen.getAllByText(/^site-checkout#/, { selector: '.work-row .ref' })
      expect(rows()).toHaveLength(7)
      await user.click(screen.getByRole('button', { name: 'Show 7 more' }))
      expect(rows()).toHaveLength(14)
    })
  })
})
