import { screen, within } from '@testing-library/react'
import { renderApp } from '../test/render'

const labels = (menu: HTMLElement): (string | null)[] => within(menu).getAllByRole('link').map((link) => link.textContent)

describe('the menus', () => {
  it('has one entry for each group, so a phone shows four', async () => {
    renderApp({ path: '/desk' })
    const main = await screen.findByRole('navigation', { name: 'Main menu' })
    expect(labels(main).map((text) => text?.replace(/\d+$/, ''))).toEqual(['Decisions', 'Overview', 'Organization', 'Results'])
  })

  it('opens a group at its first page, and lists the pages of the group in a menu of its own', async () => {
    renderApp({ path: '/people' })
    const main = await screen.findByRole('navigation', { name: 'Main menu' })
    expect(within(main).getByRole('link', { name: 'Organization' })).toHaveAttribute('aria-current', 'page')
    expect(within(main).getByRole('link', { name: 'Organization' }).getAttribute('href')).toBe('/org')
    const section = screen.getByRole('navigation', { name: 'Organization pages' })
    expect(labels(section)).toEqual(['Org chart', 'Participants and work'])
    expect(within(section).getByRole('link', { name: 'Participants and work' })).toHaveAttribute('aria-current', 'page')
    expect(within(section).getByRole('link', { name: 'Org chart' })).not.toHaveAttribute('aria-current')
  })

  it('keeps the group current on each of its pages', async () => {
    renderApp({ path: '/costs' })
    const main = await screen.findByRole('navigation', { name: 'Main menu' })
    expect(within(main).getByRole('link', { name: 'Results' })).toHaveAttribute('aria-current', 'page')
    expect(within(main).getByRole('link', { name: 'Organization' })).not.toHaveAttribute('aria-current')
    expect(labels(screen.getByRole('navigation', { name: 'Results pages' }))).toEqual(['Costs', 'Evaluation', 'Live'])
  })

  it('has no menu of its own on a page that is a group by itself', async () => {
    renderApp({ path: '/overview' })
    const main = await screen.findByRole('navigation', { name: 'Main menu' })
    expect(within(main).getByRole('link', { name: 'Overview' })).toHaveAttribute('aria-current', 'page')
    expect(screen.queryByRole('navigation', { name: /pages$/ })).not.toBeInTheDocument()
  })

  it('reads in Japanese', async () => {
    renderApp({ path: '/org', locale: 'ja' })
    const main = await screen.findByRole('navigation', { name: 'メインメニュー' })
    expect(labels(main).map((text) => text?.replace(/\d+$/, ''))).toEqual(['あなた宛てのリクエスト', 'チームの状況', '組織', '成果'])
    expect(labels(screen.getByRole('navigation', { name: '組織のページ' }))).toEqual(['組織図', 'メンバーの作業状況'])
  })
})
