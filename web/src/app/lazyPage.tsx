import { Component, lazy, Suspense, type ComponentProps, type ComponentType, type ReactElement, type ReactNode } from 'react'
import { useI18n } from '../i18n'
import { Loading, Skeleton } from '../shared/Skeleton'

/** The page's own files could not be fetched: not the page failing to draw, which is not this module's to catch. */
class PageLoadError extends Error {
  constructor(readonly reason: unknown) {
    super('The page could not be loaded')
  }
}

/**
 * Set when the build tool says a file it was to fetch is gone (`vite:preloadError`): after the node has updated itself, a tab that is
 * still open asks for the files of the version it was served, and the node answers a name it no longer has with the page.
 */
let isOutOfDate = false
if (typeof window !== 'undefined') window.addEventListener('vite:preloadError', () => (isOutOfDate = true))

function Failed({ failures, onTryAgain }: { failures: number; onTryAgain: () => void }) {
  const { t } = useI18n()
  const text = t.app.pageLoad
  const mayBeNewVersion = failures >= 2 || isOutOfDate
  return (
    <p className="empty" role="alert">
      {mayBeNewVersion ? text.newVersion : text.failed}{' '}
      {mayBeNewVersion ? (
        <button className="btn primary" type="button" onClick={() => window.location.reload()}>
          {text.reload}
        </button>
      ) : (
        <button className="btn" type="button" onClick={onTryAgain}>
          {text.tryAgain}
        </button>
      )}
    </p>
  )
}

interface BoundaryProps {
  /** What is drawn, asked for each time it is drawn, so that a retry draws the page it has just asked for again. */
  children: () => ReactNode
  /** Makes the page fetchable again: a failed fetch is remembered by the component that made it. */
  onRetry: () => void
}

class LoadFailure extends Component<BoundaryProps, { error: unknown; failures: number }> {
  state = { error: null as unknown, failures: 0 }

  static getDerivedStateFromError(error: unknown): { error: unknown } {
    return { error }
  }

  componentDidCatch(error: unknown): void {
    if (error instanceof PageLoadError) this.setState((now) => ({ failures: now.failures + 1 }))
  }

  render(): ReactNode {
    const { error, failures } = this.state
    if (error === null) return this.props.children()
    // What is not a failure to fetch the page (the page drew badly) goes on up, to whoever catches it.
    if (!(error instanceof PageLoadError)) throw error
    return (
      <Failed
        failures={failures}
        onTryAgain={() => {
          this.props.onRetry()
          this.setState({ error: null })
        }}
      />
    )
  }
}

/**
 * A page that is fetched when it is first opened, not with every page: the script of a page nobody has opened is not downloaded,
 * parsed, or run. While it comes, it is a skeleton in its place, as any region that is waiting on something. It takes the
 * props of the page it stands for. A page that cannot be fetched says so, with "Try again", which fetches it through a new lazy
 * component, since `React.lazy` keeps the failure it had; a second failure, or the build tool's word that a file is gone, says a new
 * version may have been installed, and offers to reload.
 */
export function lazyPage<Page extends ComponentType<any>>(load: () => Promise<{ default: Page }>): (props: ComponentProps<Page>) => ReactElement {
  const fetchPage = () =>
    lazy(() =>
      load().catch((reason: unknown) => {
        throw new PageLoadError(reason)
      }),
    )
  let Lazy = fetchPage()
  return function LazyPage(props) {
    return (
      <LoadFailure
        onRetry={() => {
          Lazy = fetchPage()
        }}
      >
        {() => (
          <Suspense
            fallback={
              <Loading>
                <Skeleton kind="title" />
                <Skeleton />
              </Loading>
            }
          >
            <Lazy {...(props as ComponentProps<Page>)} />
          </Suspense>
        )}
      </LoadFailure>
    )
  }
}
