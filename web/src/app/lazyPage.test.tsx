import { fireEvent, render, screen } from '@testing-library/react'
import { Component, type ReactElement, type ReactNode } from 'react'
import { I18nProvider } from '../i18n'
import { lazyPage } from './lazyPage'

function Greeting({ name }: { name: string }) {
  return <p>Hello, {name}</p>
}

const draw = (Page: (props: { name: string }) => ReactElement, locale: 'en' | 'ja' = 'en') =>
  render(
    <I18nProvider initial={locale}>
      <Page name="Alice" />
    </I18nProvider>,
  )

describe('a page that is fetched when it is first opened', () => {
  it('is not fetched when it is stood for, only when it is first drawn', async () => {
    const load = vi.fn(() => Promise.resolve({ default: Greeting }))
    const Page = lazyPage(load)
    expect(load).not.toHaveBeenCalled()
    draw(Page)
    expect(await screen.findByText('Hello, Alice')).toBeInTheDocument()
    expect(load).toHaveBeenCalledTimes(1)
  })

  it('is a skeleton in its place while it comes, and then the page with its props', async () => {
    let arrive: (page: { default: typeof Greeting }) => void = () => undefined
    const Page = lazyPage(() => new Promise<{ default: typeof Greeting }>((resolve) => (arrive = resolve)))
    draw(Page)
    expect(screen.getByText('Loading…')).toBeInTheDocument()
    expect(document.querySelectorAll('.skeleton')).toHaveLength(2)
    expect(screen.queryByText(/Hello/)).not.toBeInTheDocument()
    arrive({ default: Greeting })
    expect(await screen.findByText('Hello, Alice')).toBeInTheDocument()
    expect(screen.queryByText('Loading…')).not.toBeInTheDocument()
    expect(document.querySelectorAll('.skeleton')).toHaveLength(0)
  })

  it('is fetched once, however many times it is drawn', async () => {
    const load = vi.fn(() => Promise.resolve({ default: Greeting }))
    const Page = lazyPage(load)
    const first = draw(Page)
    await screen.findByText('Hello, Alice')
    first.unmount()
    draw(Page)
    expect(await screen.findByText('Hello, Alice')).toBeInTheDocument()
    expect(load).toHaveBeenCalledTimes(1)
  })
})

describe('a page that cannot be fetched', () => {
  let reload: ReturnType<typeof vi.fn>
  const original = window.location

  beforeEach(() => {
    // The page asks for its files again after a failure, and the failure is logged by React; neither is what these tests are about.
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    reload = vi.fn()
    Object.defineProperty(window, 'location', { configurable: true, value: { ...original, reload } })
  })
  afterEach(() => {
    Object.defineProperty(window, 'location', { configurable: true, value: original })
    vi.restoreAllMocks()
  })

  it('says so, with "Try again", and fetches it through a new import, so that the failure React kept is not the answer', async () => {
    const load = vi.fn<() => Promise<{ default: typeof Greeting }>>().mockRejectedValueOnce(new Error('Failed to fetch dynamically imported module')).mockResolvedValue({ default: Greeting })
    draw(lazyPage(load))
    expect(await screen.findByRole('alert')).toHaveTextContent('This page could not be loaded.')
    expect(screen.queryByText(/Failed to fetch/)).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Reload' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByText('Hello, Alice')).toBeInTheDocument()
    expect(load).toHaveBeenCalledTimes(2)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('says, at a second failure, that a new version may have been installed, and offers to reload, not to try again', async () => {
    const load = vi.fn<() => Promise<{ default: typeof Greeting }>>().mockRejectedValue(new Error('Failed to fetch dynamically imported module'))
    draw(lazyPage(load))
    await screen.findByRole('button', { name: 'Try again' })
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(await screen.findByText(/A new version of Agency may have been installed/)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Try again' })).not.toBeInTheDocument()
    expect(load).toHaveBeenCalledTimes(2)
    fireEvent.click(screen.getByRole('button', { name: 'Reload' }))
    expect(reload).toHaveBeenCalledTimes(1)
  })

  it('says it in Japanese, both times', async () => {
    const load = vi.fn<() => Promise<{ default: typeof Greeting }>>().mockRejectedValue(new Error('x'))
    draw(lazyPage(load), 'ja')
    expect(await screen.findByRole('alert')).toHaveTextContent('このページを読み込めませんでした。')
    fireEvent.click(screen.getByRole('button', { name: 'もう一度試す' }))
    expect(await screen.findByText(/新しい版の Agency が入った可能性があり/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '再読み込み' })).toBeInTheDocument()
  })

  it('says a new version may have been installed at once when the build tool says a file is gone', async () => {
    const load = vi.fn<() => Promise<{ default: typeof Greeting }>>().mockRejectedValue(new Error('x'))
    draw(lazyPage(load))
    window.dispatchEvent(new Event('vite:preloadError'))
    expect(await screen.findByText(/A new version of Agency may have been installed/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Reload' })).toBeInTheDocument()
  })

  it('does not take a page that failed to draw for one that failed to come: it goes up to whoever catches it', async () => {
    class Catcher extends Component<{ children: ReactNode }, { message: string | null }> {
      state = { message: null as string | null }
      static getDerivedStateFromError(error: Error) {
        return { message: error.message }
      }
      render() {
        return this.state.message === null ? this.props.children : <p>caught: {this.state.message}</p>
      }
    }
    function Broken(): ReactElement {
      throw new Error('it drew badly')
    }
    const Page = lazyPage(() => Promise.resolve({ default: Broken as unknown as typeof Greeting }))
    render(
      <I18nProvider initial="en">
        <Catcher>
          <Page name="Alice" />
        </Catcher>
      </I18nProvider>,
    )
    expect(await screen.findByText('caught: it drew badly')).toBeInTheDocument()
    expect(screen.queryByText('This page could not be loaded.')).not.toBeInTheDocument()
  })
})

describe('the pages of the router', () => {
  it('are fetched when opened: the owner’s pages are not in the script every page loads', async () => {
    const { readFileSync } = await import('node:fs')
    const source = readFileSync(`${process.cwd()}/src/app/router.tsx`, 'utf8')
    for (const page of ['OverviewPage', 'PeoplePage', 'OrgChartPage', 'CostsPage', 'EvaluationPage', 'OutcomesPage', 'LivePage']) {
      expect(source, page).toMatch(new RegExp(`const ${page} = lazyPage\\(\\(\\) => import\\('`))
      expect(source, `${page} is not imported as itself`).not.toMatch(new RegExp(`^import \\{[^}]*\\b${page}\\b[^}]*\\} from`, 'm'))
    }
  })
})
