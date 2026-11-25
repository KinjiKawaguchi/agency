import { Outlet, createRootRoute, createRoute, createRouter, redirect, type RouterHistory } from '@tanstack/react-router'
import { CostsPage } from '../costs/CostsPage'
import { DeskPage } from '../desk/DeskPage'
import { OrgChartPage } from '../org/OrgChartPage'
import { PeoplePage } from '../people/PeoplePage'
import { SignInExpiredBanner } from '../shared/SignInExpired'
import { AppBar } from './AppBar'

function Shell() {
  return (
    <>
      <AppBar />
      <main className="page">
        <SignInExpiredBanner />
        <Outlet />
      </main>
    </>
  )
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

const peopleRoute = createRoute({ getParentRoute: () => rootRoute, path: '/people', component: PeoplePage })
const nonEmpty = (value: unknown): string | undefined => (typeof value === 'string' && value !== '' ? value : undefined)

const orgRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/org',
  validateSearch: (search: Record<string, unknown>): { org?: string; unit?: string } => ({
    ...(nonEmpty(search.org) === undefined ? {} : { org: nonEmpty(search.org) }),
    ...(nonEmpty(search.unit) === undefined ? {} : { unit: nonEmpty(search.unit) }),
  }),
  component: function OrgRoute() {
    const { org, unit } = orgRoute.useSearch()
    return <OrgChartPage org={org} unit={unit} />
  },
})
const costsRoute = createRoute({ getParentRoute: () => rootRoute, path: '/costs', component: CostsPage })

const routeTree = rootRoute.addChildren([indexRoute, deskRoute, peopleRoute, orgRoute, costsRoute])

export function createAppRouter(history?: RouterHistory) {
  return createRouter({ routeTree, ...(history === undefined ? {} : { history }) })
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>
  }
}
