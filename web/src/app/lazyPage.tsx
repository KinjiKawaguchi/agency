import { lazy, Suspense, type ComponentProps, type ComponentType, type ReactElement } from 'react'
import { Loading, Skeleton } from '../shared/Skeleton'

/**
 * A page that is fetched when it is first opened, not with every page: the script of a page nobody has opened is not downloaded,
 * parsed, or run. While it comes, it is a skeleton in its place, as any region that is waiting on something. It takes the
 * props of the page it stands for.
 */
export function lazyPage<Page extends ComponentType<any>>(load: () => Promise<{ default: Page }>): (props: ComponentProps<Page>) => ReactElement {
  const Lazy = lazy(load)
  return function LazyPage(props) {
    return (
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
    )
  }
}
