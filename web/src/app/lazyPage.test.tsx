import { render, screen } from '@testing-library/react'
import { I18nProvider } from '../i18n'
import { lazyPage } from './lazyPage'

function Greeting({ name }: { name: string }) {
  return <p>Hello, {name}</p>
}

const draw = (Page: (props: { name: string }) => React.ReactElement) =>
  render(
    <I18nProvider initial="en">
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
