import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { renderApp } from '../test/render'
import { en } from './en'
import { createFormatters } from './format'
import { ja } from './ja'

/** Dotted paths of every leaf in a catalog, with the kind of leaf: text, function, or pair. */
function leaves(value: unknown, path = ''): string[] {
  if (typeof value === 'string') return [`${path}:text`]
  if (typeof value === 'function') return [`${path}:function`]
  if (Array.isArray(value)) return [`${path}:pair`]
  return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) => leaves(child, path === '' ? key : `${path}.${key}`))
}

describe('message catalogs', () => {
  it('give the same keys, of the same kind, in every locale', () => {
    expect(leaves(ja)).toEqual(leaves(en))
  })

  it('have no empty text', () => {
    const empty = (catalog: unknown, path = ''): string[] =>
      typeof catalog === 'string'
        ? catalog === '' ? [path] : []
        : typeof catalog === 'function' || Array.isArray(catalog)
          ? []
          : Object.entries(catalog as Record<string, unknown>).flatMap(([key, child]) => empty(child, `${path}.${key}`))
    expect([...empty(en), ...empty(ja)]).toEqual([])
  })
})

describe('formatters', () => {
  const now = Date.parse('2026-01-10T12:00:00Z')

  it('writes relative times in the locale', () => {
    expect(createFormatters('en', () => now).relative('2026-01-10T09:00:00Z', 'just now')).toBe('3 hours ago')
    expect(createFormatters('ja', () => now).relative('2026-01-10T09:00:00Z', 'たった今')).toBe('3 時間前')
  })

  it('says "just now" under a minute', () => {
    expect(createFormatters('en', () => now).relative('2026-01-10T11:59:40Z', 'just now')).toBe('just now')
  })
})

describe('locale', () => {
  it('shows Japanese when asked and switches back from the language menu', async () => {
    const user = userEvent.setup()
    renderApp({ locale: 'ja' })
    await waitFor(() => expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('1 人の参加者があなたの返事を待っています。'))
    expect(document.documentElement.lang).toBe('ja')
    await user.selectOptions(screen.getByRole('combobox', { name: '言語' }), 'en')
    await waitFor(() => expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('1 participant is waiting for your answer.'))
    expect(document.documentElement.lang).toBe('en')
    expect(window.localStorage.getItem('agency.locale')).toBe('en')
  })

  it('labels the options in Japanese too', async () => {
    renderApp({ locale: 'ja' })
    const pile = within(await screen.findByRole('listbox', { name: '届いている書類' }))
    expect(pile.getByText('決裁する稟議')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '決裁する' })).toBeInTheDocument()
  })

  it('says a comment is a comment in Japanese, on the button and in the result', async () => {
    const user = userEvent.setup()
    renderApp({ locale: 'ja' })
    await user.type(await screen.findByLabelText(/^補足/), '十二月からにできませんか。')
    await user.click(screen.getByRole('button', { name: 'コメントを送る' }))
    expect(await screen.findByText(/さんがコメントしました。/)).toBeInTheDocument()
    expect(screen.getByText(/^コメントしました。.* に届きます。まだ決裁はされていません。$/)).toBeInTheDocument()
  })
})
