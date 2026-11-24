import { Outlet, createRootRoute, createRoute, createRouter, redirect, type RouterHistory } from '@tanstack/react-router'
import { DeskPage } from '../desk/DeskPage'
import { useI18n } from '../i18n'
import { AppBar } from './AppBar'

function Shell() {
  return (
    <>
      <AppBar />
      <main className="page">
        <Outlet />
      </main>
    </>
  )
}

function ComingSoon() {
  const { t } = useI18n()
  return <p className="page-soon">{t.nav.comingSoon}</p>
}

const rootRoute = createRootRoute({ component: Shell })

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: '/desk', replace: true })
  },
})

const deskRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/desk',
  validateSearch: (search: Record<string, unknown>): { item?: string } =>
    typeof search.item === 'string' && search.item !== '' ? { item: search.item } : {},
  component: function DeskRoute() {
    const { item } = deskRoute.useSearch()
    const navigate = deskRoute.useNavigate()
    return <DeskPage selected={item} onSelect={(key) => void navigate({ search: { item: key }, replace: true })} />
  },
})

const peopleRoute = createRoute({ getParentRoute: () => rootRoute, path: '/people', component: ComingSoon })
const orgRoute = createRoute({ getParentRoute: () => rootRoute, path: '/org', component: ComingSoon })
const costsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/costs', component: ComingSoon })

const routeTree = rootRoute.addChildren([indexRoute, deskRoute, peopleRoute, orgRoute, costsRoute])

export function createAppRouter(history?: RouterHistory) {
  return createRouter({ routeTree, ...(history === undefined ? {} : { history }) })
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>
  }
}
