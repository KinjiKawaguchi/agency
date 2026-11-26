import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import { createTestNode } from '../test/testDevice'

const NATIVE = 'agencyNative'

async function open() {
  renderApp({ path: '/devices' })
  return within(await screen.findByRole('region', { name: 'Home nodes' }))
}

describe('home nodes on the devices screen', () => {
  afterEach(() => {
    Reflect.deleteProperty(window, NATIVE)
  })

  it('lists no other node at first, and shows no name in the app bar', async () => {
    const card = await open()
    expect(card.getByText('You have listed no other home node.')).toBeInTheDocument()
    expect(document.querySelector('.node-chip')).toBeNull()
  })

  it('adds a node as a link to its own origin, and keeps it after a reload of the page', async () => {
    const user = userEvent.setup()
    const card = await open()
    await user.type(card.getByLabelText('Name'), 'Work')
    await user.type(card.getByLabelText('Address'), 'https://work.example.com/some/path')
    await user.click(card.getByRole('button', { name: 'Add this home node' }))
    const link = card.getByRole('link', { name: 'Work' })
    expect(link).toHaveAttribute('href', 'https://work.example.com')
    expect(link).toHaveAttribute('rel', 'noopener noreferrer')
    expect(card.getByLabelText('Name')).toHaveValue('')
    expect(window.localStorage.getItem('agency.homeNodes')).toContain('https://work.example.com')
  })

  it('says what is wrong with an address and adds nothing', async () => {
    const user = userEvent.setup()
    const card = await open()
    await user.type(card.getByLabelText('Address'), 'http://work.example.com')
    await user.click(card.getByRole('button', { name: 'Add this home node' }))
    expect(await card.findByRole('alert')).toHaveTextContent('Use https. Only a node on this machine may use http.')
    expect(card.getByText('You have listed no other home node.')).toBeInTheDocument()
  })

  it('removes a node from the list', async () => {
    const user = userEvent.setup()
    window.localStorage.setItem('agency.homeNodes', JSON.stringify({ others: [{ label: 'Work', origin: 'https://work.example.com' }] }))
    const card = await open()
    await user.click(card.getByRole('button', { name: 'Remove Work from the list' }))
    expect(card.queryByRole('link', { name: 'Work' })).not.toBeInTheDocument()
    expect(card.getByText('You have listed no other home node.')).toBeInTheDocument()
  })

  it('names this node in the app bar once another one is listed, by its host name until it is named', async () => {
    const user = userEvent.setup()
    window.localStorage.setItem('agency.homeNodes', JSON.stringify({ others: [{ label: 'Work', origin: 'https://work.example.com' }] }))
    const card = await open()
    expect(document.querySelector('.node-chip')).toHaveTextContent(window.location.host)
    await user.type(card.getByLabelText('Name of this home node'), 'Personal')
    expect(document.querySelector('.node-chip')).toHaveTextContent('Personal')
  })

  it('shows nothing in the iPhone app, whose own switcher owns the nodes', async () => {
    Object.defineProperty(window, NATIVE, { value: { platform: 'ios', push: null }, configurable: true })
    window.localStorage.setItem('agency.homeNodes', JSON.stringify({ others: [{ label: 'Work', origin: 'https://work.example.com' }] }))
    renderApp({ path: '/devices' })
    await screen.findByRole('heading', { level: 1, name: 'Devices' })
    expect(screen.queryByRole('region', { name: 'Home nodes' })).not.toBeInTheDocument()
    expect(document.querySelector('.node-chip')).toBeNull()
  })

  it('reads in Japanese', async () => {
    renderApp({ path: '/devices', locale: 'ja' })
    const card = within(await screen.findByRole('region', { name: 'ホームノード' }))
    expect(card.getByText('一覧に、ほかのホームノードはありません。')).toBeInTheDocument()
    expect(card.getByRole('button', { name: 'このホームノードを追加' })).toBeInTheDocument()
  })

  describe('the nodes the node’s configuration names', () => {
    const node = () => createTestNode({ homeNodes: [{ label: 'Work', url: 'https://work.example.com' }] })

    it('are listed as links without a way to remove them, said to come from the node’s configuration, and name this node in the app bar', async () => {
      renderApp({ path: '/devices', node: node() })
      const card = within(await screen.findByRole('region', { name: 'Home nodes' }))
      const link = await card.findByRole('link', { name: 'Work' })
      expect(link).toHaveAttribute('href', 'https://work.example.com')
      expect(card.getByText('Set in this node’s configuration')).toBeInTheDocument()
      expect(card.queryByRole('button', { name: 'Remove Work from the list' })).not.toBeInTheDocument()
      expect(card.queryByText('You have listed no other home node.')).not.toBeInTheDocument()
      expect(document.querySelector('.node-chip')).not.toBeNull()
    })

    it('come before the ones the owner adds here, which can still be removed', async () => {
      const user = userEvent.setup()
      renderApp({ path: '/devices', node: node() })
      const card = within(await screen.findByRole('region', { name: 'Home nodes' }))
      await card.findByRole('link', { name: 'Work' })
      await user.type(card.getByLabelText('Name'), 'Lab')
      await user.type(card.getByLabelText('Address'), 'https://lab.example.com')
      await user.click(card.getByRole('button', { name: 'Add this home node' }))
      expect(card.getAllByRole('link').map((one) => one.textContent)).toEqual(['Work', 'Lab'])
      await user.click(card.getByRole('button', { name: 'Remove Lab from the list' }))
      expect(card.queryByRole('link', { name: 'Lab' })).not.toBeInTheDocument()
      expect(card.getByRole('link', { name: 'Work' })).toBeInTheDocument()
    })

    it('are listed once when the owner added the same one by hand', async () => {
      const user = userEvent.setup()
      renderApp({ path: '/devices', node: node() })
      const card = within(await screen.findByRole('region', { name: 'Home nodes' }))
      await card.findByRole('link', { name: 'Work' })
      await user.type(card.getByLabelText('Name'), 'My work')
      await user.type(card.getByLabelText('Address'), 'https://work.example.com/')
      await user.click(card.getByRole('button', { name: 'Add this home node' }))
      expect(card.getAllByRole('link').map((one) => one.textContent)).toEqual(['Work'])
    })
  })
})
